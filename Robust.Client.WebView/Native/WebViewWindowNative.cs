using System;
using System.Collections.Generic;
using Robust.Shared.ViewVariables;

namespace Robust.Client.WebView.Native;

internal sealed class WebViewWindowNative : IWebViewWindow
{
    private readonly WebViewManagerNative _manager;
    private readonly nint _handle;
    private readonly List<Action<IRequestHandlerContext>> _requestHandlers = new();
    private string _currentUrl = "about:blank";

    public WebViewWindowNative(WebViewManagerNative manager, nint handle)
    {
        _manager = manager;
        _handle = handle;
    }

    [ViewVariables(VVAccess.ReadWrite)]
    public string Url
    {
        get
        {
            CheckClosed();
            return _currentUrl;
        }
        set
        {
            CheckClosed();
            _currentUrl = value;
            WebViewNative.robust_webview_navigate(_handle, value);
        }
    }

    [ViewVariables]
    public bool IsLoading
    {
        get
        {
            CheckClosed();
            return WebViewNative.robust_webview_is_loading(_handle);
        }
    }

    public void StopLoad()
    {
        CheckClosed();
        WebViewNative.robust_webview_stop(_handle);
    }

    public void Reload()
    {
        CheckClosed();
        WebViewNative.robust_webview_reload(_handle);
    }

    public bool GoBack()
    {
        CheckClosed();
        if (!WebViewNative.robust_webview_can_go_back(_handle))
            return false;

        WebViewNative.robust_webview_go_back(_handle);
        return true;
    }

    public bool GoForward()
    {
        CheckClosed();
        if (!WebViewNative.robust_webview_can_go_forward(_handle))
            return false;

        WebViewNative.robust_webview_go_forward(_handle);
        return true;
    }

    public void ExecuteJavaScript(string code)
    {
        CheckClosed();
        WebViewNative.robust_webview_execute_js(_handle, code);
    }

    public void AddResourceRequestHandler(Action<IRequestHandlerContext> handler)
    {
        _requestHandlers.Add(handler);
    }

    public void RemoveResourceRequestHandler(Action<IRequestHandlerContext> handler)
    {
        _requestHandlers.Remove(handler);
    }

    public void Dispose()
    {
        if (Closed)
            return;

        WebViewNative.robust_webview_destroy(_handle);
        Closed = true;
        _manager.RemoveWindow(this);
    }

    public bool Closed { get; private set; }

    private void CheckClosed()
    {
        if (Closed)
            throw new ObjectDisposedException(nameof(WebViewWindowNative));
    }
}
