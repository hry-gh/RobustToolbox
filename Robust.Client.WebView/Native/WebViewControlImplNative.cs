using System;
using System.Collections.Generic;
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
            _url = value;
            if (_handle != 0)
                WebViewNative.robust_webview_navigate(_handle, value);
        }
    }

    public bool IsLoading
    {
        get => _handle != 0 && WebViewNative.robust_webview_is_loading(_handle);
    }

    public void StartBrowser()
    {
        if (_handle != 0)
            return;

        var window = _owner.Window;
        var parentHandle = GetOwnerWindowHandle();
        Sawmill.Info($"StartBrowser: owner.Window={window} (type={window?.GetType().Name}), " +
                     $"ownerHandle=0x{parentHandle:X}");

        if (parentHandle == 0)
        {
            parentHandle = _manager.GetMainWindowHandle();
            Sawmill.Info($"StartBrowser: fell back to main window handle=0x{parentHandle:X}");
        }

        _handle = WebViewNative.robust_webview_create(parentHandle, _url);
        Sawmill.Info($"StartBrowser: created webview handle=0x{_handle:X}, url={_url}");

        if (_handle != 0)
            UpdateSizeAndPosition();
    }

    public void CloseBrowser()
    {
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
        UpdateSizeAndPosition();
    }

    public void Draw(DrawingHandleScreen handle)
    {
        // Native webview renders itself as an OS overlay - nothing to draw here.
        // Just ensure position is up to date in case the control moved.
        UpdateSizeAndPosition();
    }

    private nint GetOwnerWindowHandle()
    {
        // Walk up the UI tree to find which IClydeWindow this control lives in
        var window = _owner.Window;
        Sawmill.Debug($"GetOwnerWindowHandle: window={window}, isInternal={window is IClydeWindowInternal}");

        if (window is IClydeWindowInternal windowInternal)
        {
            var hwnd = windowInternal.WindowsHWnd;
            var cocoa = windowInternal.CocoaWindow;
            var x11 = windowInternal.X11Id;
            Sawmill.Debug($"GetOwnerWindowHandle: WindowsHWnd=0x{hwnd ?? 0:X}, " +
                          $"CocoaWindow=0x{cocoa ?? 0:X}, X11Id={x11 ?? 0}");

            if (OperatingSystem.IsWindows())
                return hwnd ?? 0;
            if (OperatingSystem.IsMacOS())
                return cocoa ?? 0;
            if (OperatingSystem.IsLinux())
                return (nint)(x11 ?? 0);
        }

        return 0;
    }

    private bool _boundsLogged;

    private void UpdateSizeAndPosition()
    {
        if (_handle == 0)
            return;

        var pos = _owner.GlobalPixelPosition;
        var width = _owner.PixelWidth;
        var height = _owner.PixelHeight;

        if (!_boundsLogged)
        {
            Sawmill.Info($"UpdateBounds: pos=({pos.X},{pos.Y}), size=({width}x{height})");
            _boundsLogged = true;
        }

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
}
