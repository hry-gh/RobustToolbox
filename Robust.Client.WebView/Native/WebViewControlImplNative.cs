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
                NavigateOrIntercept(value);
        }
    }

    private void NavigateOrIntercept(string url)
    {
        // Try to serve via registered request handlers (e.g. http://127.0.0.1/ fake URLs)
        if (_requestHandlers.Count > 0 && TryHandleViaRequestHandlers(url))
            return;

        WebViewNative.robust_webview_navigate(_handle, url);
    }

    private bool TryHandleViaRequestHandlers(string url)
    {
        var context = new NativeRequestHandlerContext(url);

        foreach (var handler in _requestHandlers)
        {
            handler(context);
            if (context.IsHandled)
            {
                if (context.ResponseStream != null)
                {
                    using var reader = new StreamReader(context.ResponseStream);
                    var content = reader.ReadToEnd();
                    Sawmill.Debug($"Request handler served {url} ({content.Length} chars, {context.ResponseMimeType})");
                    WebViewNative.robust_webview_load_html(_handle, content, url);
                }

                return true;
            }

            if (context.IsCancelled)
                return true;
        }

        return false;
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
        {
            Sawmill.Debug($"TryCreate: deferred (parent=0x{parentHandle:X}, size={width}x{height})");
            return;
        }

        // Create with about:blank first, then navigate via request handler intercept
        _handle = WebViewNative.robust_webview_create(parentHandle, null);
        Sawmill.Info($"TryCreate: created handle=0x{_handle:X}, parent=0x{parentHandle:X}, url={_url}");

        if (_handle != 0)
        {
            var pos = _owner.GlobalPixelPosition;
            Sawmill.Info($"TryCreate: bounds=({pos.X},{pos.Y},{width}x{height})");
            WebViewNative.robust_webview_set_bounds(_handle, pos.X, pos.Y, width, height);
            NavigateOrIntercept(_url);
        }
    }

    public void CloseBrowser()
    {
        _wantOpen = false;

        if (_handle == 0)
            return;

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

    public void AddBeforeBrowseHandler(Action<IBeforeBrowseContext> handler) { }
    public void RemoveBeforeBrowseHandler(Action<IBeforeBrowseContext> handler) { }

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
        }
    }
}
