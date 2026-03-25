using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using Robust.Client.Graphics;
using Robust.Client.UserInterface;
using Robust.Shared.Log;

namespace Robust.Client.WebView.Native;

/// <summary>
/// Native webview control implementation. Overlays a real OS webview on top of the game window,
/// positioned to match the UI control's screen location.
/// </summary>
internal sealed class WebViewControlImplNative : IWebViewControlImpl
{
    private readonly WebViewManagerNative _manager;
    private readonly WebViewControl _owner;
    private nint _handle;
    private string _url = "about:blank";
    private bool _wantOpen;
    private readonly List<Action<IRequestHandlerContext>> _requestHandlers = new();
    private static readonly ISawmill Sawmill = Logger.GetSawmill("web.native.control");

    public WebViewControlImplNative(WebViewManagerNative manager, WebViewControl owner)
    {
        _manager = manager;
        _owner = owner;
    }

    public bool IsOpen => _handle != 0;

    public string Url
    {
        get => _url;
        set
        {
            Sawmill.Debug($"Url set: '{value}' (handle=0x{_handle:X})");
            _url = value;
            if (_handle != 0)
            {
                var nativeUrl = RewriteUrlForNative(value);
                WebViewNative.robust_webview_navigate(_handle, nativeUrl);
            }
        }
    }

    public bool IsLoading
    {
        get => _handle != 0 && WebViewNative.robust_webview_is_loading(_handle);
    }

    public void StartBrowser()
    {
        _wantOpen = true;
        TryCreateWebView();
    }

    private void TryCreateWebView()
    {
        if (_handle != 0 || !_wantOpen)
            return;

        var parentHandle = GetOwnerWindowHandle();
        var width = _owner.PixelWidth;
        var height = _owner.PixelHeight;

        // Defer until we have a valid window and non-zero size
        if (parentHandle == 0 || width <= 0 || height <= 0)
            return;

        // Register for scheme handling BEFORE create so the initial load can be served
        _manager.RegisterControl(this);

        var nativeUrl = RewriteUrlForNative(_url);
        _handle = WebViewNative.robust_webview_create(parentHandle, nativeUrl);
        Sawmill.Info($"Created webview handle=0x{_handle:X}, parent=0x{parentHandle:X}, url={nativeUrl}");

        if (_handle != 0)
        {
            _manager.RegisterControlHandle(_handle, this);

            var pos = _owner.GlobalPixelPosition;
            WebViewNative.robust_webview_set_bounds(_handle, pos.X, pos.Y, width, height);
        }
        else
        {
            _manager.UnregisterControl(0, this);
        }
    }

    public void CloseBrowser()
    {
        _wantOpen = false;

        if (_handle == 0)
            return;

        _manager.UnregisterControl(_handle, this);
        WebViewNative.robust_webview_destroy(_handle);
        _handle = 0;
    }

    public void StopLoad()
    {
        if (_handle != 0)
            WebViewNative.robust_webview_stop(_handle);
    }

    public void Reload()
    {
        if (_handle != 0)
            WebViewNative.robust_webview_reload(_handle);
    }

    public bool GoBack()
    {
        if (_handle == 0 || !WebViewNative.robust_webview_can_go_back(_handle))
            return false;

        WebViewNative.robust_webview_go_back(_handle);
        return true;
    }

    public bool GoForward()
    {
        if (_handle == 0 || !WebViewNative.robust_webview_can_go_forward(_handle))
            return false;

        WebViewNative.robust_webview_go_forward(_handle);
        return true;
    }

    public void ExecuteJavaScript(string code)
    {
        if (_handle != 0)
            WebViewNative.robust_webview_execute_js(_handle, code);
    }

    public void Resized()
    {
        if (_handle == 0)
        {
            TryCreateWebView();
            return;
        }

        UpdateBounds();
    }

    public void Draw(DrawingHandleScreen handle)
    {
        if (_handle == 0)
        {
            TryCreateWebView();
            return;
        }

        UpdateBounds();
    }

    /// <summary>
    /// Rewrite http://127.0.0.1/ URLs to res:// so the native scheme handler can serve them.
    /// </summary>
    private static string RewriteUrlForNative(string url)
    {
        if (url.StartsWith("http://127.0.0.1/", StringComparison.OrdinalIgnoreCase))
            return "res://" + url.Substring("http://127.0.0.1".Length);

        return url;
    }

    /// <summary>
    /// Called by the manager's scheme handler to try serving a request through
    /// this control's registered request handlers.
    /// </summary>
    internal bool TryHandleSchemeRequest(string url, out Stream? stream, out string mimeType, out int statusCode)
    {
        stream = null;
        mimeType = "application/octet-stream";
        statusCode = 404;

        var context = new NativeRequestHandlerContext(url);

        foreach (var handler in _requestHandlers)
        {
            handler(context);

            if (context.IsHandled && context.ResponseStream != null)
            {
                stream = context.ResponseStream;
                mimeType = context.ResponseMimeType;
                statusCode = (int)context.ResponseStatusCode;
                return true;
            }

            if (context.IsCancelled)
                return false;
        }

        return false;
    }

    private nint GetOwnerWindowHandle()
    {
        var window = _owner.Window;
        if (window is IClydeWindowInternal windowInternal)
        {
            if (OperatingSystem.IsWindows())
                return windowInternal.WindowsHWnd ?? 0;
            if (OperatingSystem.IsMacOS())
                return windowInternal.CocoaWindow ?? 0;
            if (OperatingSystem.IsLinux())
                return (nint)(windowInternal.X11Id ?? 0);
        }

        return 0;
    }

    private void UpdateBounds()
    {
        if (_handle == 0)
            return;

        var pos = _owner.GlobalPixelPosition;
        var width = _owner.PixelWidth;
        var height = _owner.PixelHeight;

        if (width > 0 && height > 0)
            WebViewNative.robust_webview_set_bounds(_handle, pos.X, pos.Y, width, height);
    }

    // Input forwarding - native webview handles its own input since it's an OS-level view
    public void MouseMove(GUIMouseMoveEventArgs args) { }
    public void MouseExited() { }
    public void MouseWheel(GUIMouseWheelEventArgs args) { }
    public bool RawKeyEvent(in GuiRawKeyEvent guiRawEvent) => false;
    public void TextEntered(GUITextEnteredEventArgs args) { }
    public void FocusEntered() { }
    public void FocusExited() { }

    public void AddResourceRequestHandler(Action<IRequestHandlerContext> handler)
    {
        _requestHandlers.Add(handler);
    }

    public void RemoveResourceRequestHandler(Action<IRequestHandlerContext> handler)
    {
        _requestHandlers.Remove(handler);
    }

    private readonly List<Action<IBeforeBrowseContext>> _beforeBrowseHandlers = new();

    public void AddBeforeBrowseHandler(Action<IBeforeBrowseContext> handler)
    {
        _beforeBrowseHandlers.Add(handler);
    }

    public void RemoveBeforeBrowseHandler(Action<IBeforeBrowseContext> handler)
    {
        _beforeBrowseHandlers.Remove(handler);
    }

    /// <summary>
    /// Called by the manager when the native webview is about to navigate.
    /// Returns true to cancel the navigation.
    /// </summary>
    internal bool HandleBeforeBrowse(string url, bool isRedirect)
    {
        if (_beforeBrowseHandlers.Count == 0)
            return false;

        // Rewrite res:// back to http://127.0.0.1/ for content-side handlers
        var handlerUrl = url;
        if (url.StartsWith("res://", StringComparison.OrdinalIgnoreCase))
        {
            handlerUrl = "http://127.0.0.1" + new Uri(url).AbsolutePath + new Uri(url).Query;
        }

        var context = new NativeBeforeBrowseContext(handlerUrl, isRedirect);

        foreach (var handler in _beforeBrowseHandlers)
        {
            handler(context);
            if (context.IsCancelled)
                return true;
        }

        return false;
    }

    private sealed class NativeBeforeBrowseContext : IBeforeBrowseContext
    {
        public string Url { get; }
        public string Method => "GET";
        public bool IsRedirect { get; }
        public bool UserGesture => true;
        public bool IsCancelled { get; private set; }

        public NativeBeforeBrowseContext(string url, bool isRedirect)
        {
            Url = url;
            IsRedirect = isRedirect;
        }

        public void DoCancel()
        {
            IsCancelled = true;
        }
    }

    private sealed class NativeRequestHandlerContext : IRequestHandlerContext
    {
        public bool IsNavigation => true;
        public bool IsDownload => false;
        public string RequestInitiator => "";
        public string Url { get; }
        public string Method => "GET";
        public bool IsHandled { get; private set; }
        public bool IsCancelled { get; private set; }

        public Stream? ResponseStream { get; private set; }
        public string ResponseMimeType { get; private set; } = "text/html";
        public HttpStatusCode ResponseStatusCode { get; private set; } = HttpStatusCode.OK;

        public NativeRequestHandlerContext(string url)
        {
            Url = url;
        }

        public void DoCancel()
        {
            IsCancelled = true;
        }

        public void DoRespondStream(Stream stream, string contentType, HttpStatusCode code = HttpStatusCode.OK)
        {
            IsHandled = true;
            ResponseStream = stream;
            ResponseMimeType = contentType;
            ResponseStatusCode = code;
        }
    }
}
