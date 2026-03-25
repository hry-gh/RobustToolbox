using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.IO;
using System.Net;
using System.Runtime.InteropServices;
using System.Text;
using Robust.Client.Graphics;
using Robust.Shared.ContentPack;
using Robust.Shared.IoC;
using Robust.Shared.Log;
using Robust.Shared.Utility;

namespace Robust.Client.WebView.Native;

internal sealed partial class WebViewManagerNative : IWebViewManagerImpl
{
    [Dependency] private readonly IClydeInternal _clyde = default!;
    [Dependency] private readonly IResourceManagerInternal _resourceManager = default!;
    [Dependency] private readonly ILogManager _logManager = default!;

    private ISawmill _sawmill = default!;
    private readonly List<WebViewWindowNative> _browserWindows = new();
    private WebViewNative.SchemeCallback? _schemeCallbackDelegate;

    // Track active controls by native handle so scheme handler can route to per-control request handlers
    private readonly Dictionary<nint, WebViewControlImplNative> _activeControls = new();

    private readonly Dictionary<string, string> _resourceMimeTypes = new()
    {
        { "aac", "audio/aac" },
        { "avif", "image/avif" },
        { "avi", "video/x-msvideo" },
        { "bmp", "image/bmp" },
        { "css", "text/css" },
        { "gif", "image/gif" },
        { "htm", "text/html" },
        { "html", "text/html" },
        { "ico", "image/vnd.microsoft.icon" },
        { "jpeg", "image/jpeg" },
        { "jpg", "image/jpeg" },
        { "js", "text/javascript" },
        { "json", "application/json" },
        { "jsonld", "application/ld+json" },
        { "midi", "audio/midi" },
        { "mid", "audio/midi" },
        { "mjs", "text/javascript" },
        { "mp3", "audio/mpeg" },
        { "mp4", "video/mp4" },
        { "mpeg", "video/mpeg" },
        { "oga", "audio/ogg" },
        { "ogg", "audio/ogg" },
        { "ogv", "video/ogg" },
        { "ogx", "application/ogg" },
        { "opus", "audio/opus" },
        { "otf", "font/otf" },
        { "png", "image/png" },
        { "pdf", "application/pdf" },
        { "svg", "image/svg+xml" },
        { "tiff", "image/tiff" },
        { "tif", "image/tiff" },
        { "ts", "video/mp2t" },
        { "ttf", "font/ttf" },
        { "txt", "text/plain" },
        { "wav", "audio/wav" },
        { "weba", "audio/webm" },
        { "webm", "video/webm" },
        { "webp", "image/webp" },
        { "woff", "font/woff" },
        { "woff2", "font/woff2" },
        { "xhtml", "application/xhtml+xml" },
        { "xml", "application/xml" },
        { "zip", "application/zip" },
    };

    public void Initialize()
    {
        _sawmill = _logManager.GetSawmill("web.native");

        var result = WebViewNative.robust_webview_init();
        if (result != 0)
        {
            throw new InvalidOperationException($"Failed to initialize native webview: error code {result}");
        }

        // Set up scheme handler for res:// protocol
        unsafe
        {
            _schemeCallbackDelegate = OnSchemeRequest;
            WebViewNative.robust_webview_set_scheme_handler(_schemeCallbackDelegate, 0);
        }

        _sawmill.Info("Native webview initialized");
    }

    public void Update()
    {
        WebViewNative.robust_webview_pump();
    }

    public void Shutdown()
    {
        foreach (var window in _browserWindows.ToArray())
        {
            window.Dispose();
        }

        WebViewNative.robust_webview_shutdown();
        _sawmill.Info("Native webview shutdown");
    }

    public IWebViewWindow CreateBrowserWindow(BrowserWindowCreateParameters createParams)
    {
        var parentHandle = GetMainWindowHandle();

        var handle = WebViewNative.robust_webview_create(parentHandle, createParams.Url);
        if (handle == 0)
        {
            throw new InvalidOperationException("Failed to create native webview window");
        }

        WebViewNative.robust_webview_set_size(handle, createParams.Width, createParams.Height);

        var window = new WebViewWindowNative(this, handle);
        _browserWindows.Add(window);

        return window;
    }

    internal void RemoveWindow(WebViewWindowNative window)
    {
        _browserWindows.Remove(window);
    }

    internal void RegisterControl(nint handle, WebViewControlImplNative control)
    {
        _activeControls[handle] = control;
    }

    internal void UnregisterControl(nint handle)
    {
        _activeControls.Remove(handle);
    }

    internal nint GetMainWindowHandle()
    {
        var mainWindow = _clyde.MainWindow as IClydeWindowInternal;
        if (mainWindow == null)
            return 0;

        if (OperatingSystem.IsWindows())
            return mainWindow.WindowsHWnd ?? 0;

        if (OperatingSystem.IsMacOS())
            return mainWindow.CocoaWindow ?? 0;

        if (OperatingSystem.IsLinux())
            return (nint)(mainWindow.X11Id ?? 0);

        return 0;
    }

    private unsafe void OnSchemeRequest(byte* urlPtr, nint requestHandle, nint userData)
    {
        var url = Marshal.PtrToStringUTF8((nint)urlPtr) ?? "";

        _sawmill.Debug($"Scheme request: {url}");

        try
        {
            // First, try per-control request handlers.
            // We rewrite res:// back to http://127.0.0.1/ so content-side handlers
            // see the URL format they expect.
            var httpUrl = url;
            if (url.StartsWith("res://", StringComparison.OrdinalIgnoreCase))
            {
                httpUrl = "http://127.0.0.1" + new Uri(url).AbsolutePath;
            }

            foreach (var control in _activeControls.Values)
            {
                if (control.TryHandleSchemeRequest(httpUrl, out var stream, out var mimeType, out var statusCode)
                    && stream != null)
                {
                    _sawmill.Debug($"Scheme request handled by control: {url} -> {mimeType} ({statusCode})");
                    RespondWithStream(requestHandle, stream, mimeType, statusCode);
                    return;
                }
            }

            // Fall back to engine content resources
            var uri = new Uri(url);
            var resPath = new ResPath(uri.AbsolutePath);

            if (_resourceManager.TryContentFileRead(resPath, out var contentStream))
            {
                if (!TryGetResourceMimeType(resPath.Extension, out var mime))
                    mime = "application/octet-stream";

                RespondWithStream(requestHandle, contentStream, mime, 200);
                contentStream.Dispose();
            }
            else
            {
                _sawmill.Debug($"Scheme request not found: {url}");
                RespondNotFound(requestHandle);
            }
        }
        catch (Exception ex)
        {
            _sawmill.Error($"Error handling scheme request: {ex}");
            RespondError(requestHandle, ex.Message);
        }
    }

    private unsafe void RespondWithStream(nint requestHandle, Stream stream, string mimeType, int statusCode)
    {
        using var ms = new MemoryStream();
        stream.CopyTo(ms);
        var data = ms.ToArray();

        fixed (byte* dataPtr = data)
        {
            WebViewNative.robust_webview_respond_scheme(requestHandle, dataPtr, data.Length, mimeType, statusCode);
        }
    }

    private unsafe void RespondNotFound(nint requestHandle)
    {
        var data = Encoding.UTF8.GetBytes("Not found");
        fixed (byte* dataPtr = data)
        {
            WebViewNative.robust_webview_respond_scheme(requestHandle, dataPtr, data.Length, "text/plain", 404);
        }
    }

    private unsafe void RespondError(nint requestHandle, string message)
    {
        var data = Encoding.UTF8.GetBytes($"Error: {message}");
        fixed (byte* dataPtr = data)
        {
            WebViewNative.robust_webview_respond_scheme(requestHandle, dataPtr, data.Length, "text/plain", 500);
        }
    }

    public void SetResourceMimeType(string extension, string mimeType)
    {
        DebugTools.Assert(!extension.StartsWith("."), "SetResourceMimeType extension must not include starting dot.");

        lock (_resourceMimeTypes)
        {
            _resourceMimeTypes[extension] = mimeType;
        }
    }

    public bool TryGetResourceMimeType(string extension, [NotNullWhen(true)] out string? mimeType)
    {
        lock (_resourceMimeTypes)
        {
            return _resourceMimeTypes.TryGetValue(extension, out mimeType);
        }
    }

    public IWebViewControlImpl MakeControlImpl(WebViewControl owner)
    {
        return new WebViewControlImplNative(this, owner);
    }
}
