using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Robust.Client.Graphics;
using Robust.Shared.IoC;
using Robust.Shared.Log;
using Robust.Shared.ViewVariables;

namespace Robust.Client.WebView.Cef
{
    internal partial class WebViewManagerCef
    {
        [Dependency] private readonly IClydeInternal _clyde = default!;

        private readonly List<WebViewWindowImpl> _browserWindows = new();

        public IEnumerable<IWebViewWindow> AllBrowserWindows => _browserWindows;

        public unsafe IWebViewWindow CreateBrowserWindow(BrowserWindowCreateParameters createParams)
        {
            var impl = new WebViewWindowImpl(this);
            var gcHandle = GCHandle.Alloc(impl);

            var callbacks = new RnwBrowserCallbacks
            {
                UserData = (void*)GCHandle.ToIntPtr(gcHandle),
                OnBeforeBrowse = &WindowOnBeforeBrowseCallback,
                OnResourceRequest = &WindowOnResourceRequestCallback,
                OnBeforeClose = &WindowOnBeforeCloseCallback,
                // Render callbacks not needed for windowed browsers.
            };

            var urlUtf8 = MarshalStringToUtf8(createParams.Url);
            var handle = NativeWebView.rnw_window_create(
                (byte*)urlUtf8,
                createParams.Width,
                createParams.Height,
                &callbacks);
            Marshal.FreeHGlobal(urlUtf8);

            impl.BrowserHandle = handle;
            impl.GcHandle = gcHandle;
            _browserWindows.Add(impl);

            return impl;
        }

        [UnmanagedCallersOnly]
        private static unsafe int WindowOnBeforeBrowseCallback(
            void* userData, byte* urlPtr, int userGesture, int isRedirect)
        {
            // Windows don't currently use before-browse handlers.
            return 0;
        }

        [UnmanagedCallersOnly]
        private static unsafe int WindowOnResourceRequestCallback(
            void* userData, ulong requestId, byte* urlPtr, byte* methodPtr)
        {
            // Windows don't currently use resource request handlers.
            return 0;
        }

        [UnmanagedCallersOnly]
        private static unsafe void WindowOnBeforeCloseCallback(void* userData)
        {
            var handle = GCHandle.FromIntPtr((IntPtr)userData);
            if (handle.Target is WebViewWindowImpl impl)
            {
                impl.OnClose();
            }
        }

        private sealed class WebViewWindowImpl : IWebViewWindow
        {
            private readonly WebViewManagerCef _manager;
            internal ulong BrowserHandle;
            internal GCHandle GcHandle;

            [ViewVariables(VVAccess.ReadWrite)]
            public unsafe string Url
            {
                get
                {
                    CheckClosed();
                    var buf = stackalloc byte[4096];
                    var len = NativeWebView.rnw_browser_get_url(BrowserHandle, buf, 4096);
                    if (len < 0) return "";
                    return Encoding.UTF8.GetString(buf, Math.Min(len, 4095));
                }
                set
                {
                    CheckClosed();
                    var utf8 = MarshalStringToUtf8(value);
                    NativeWebView.rnw_browser_load_url(BrowserHandle, (byte*)utf8);
                    Marshal.FreeHGlobal(utf8);
                }
            }

            [ViewVariables]
            public bool IsLoading
            {
                get
                {
                    CheckClosed();
                    return NativeWebView.rnw_browser_is_loading(BrowserHandle) != 0;
                }
            }

            public WebViewWindowImpl(WebViewManagerCef manager)
            {
                _manager = manager;
            }

            public void StopLoad()
            {
                CheckClosed();
                NativeWebView.rnw_browser_stop_load(BrowserHandle);
            }

            public void Reload()
            {
                CheckClosed();
                NativeWebView.rnw_browser_reload(BrowserHandle);
            }

            public bool GoBack()
            {
                CheckClosed();
                if (NativeWebView.rnw_browser_can_go_back(BrowserHandle) == 0)
                    return false;
                NativeWebView.rnw_browser_go_back(BrowserHandle);
                return true;
            }

            public bool GoForward()
            {
                CheckClosed();
                if (NativeWebView.rnw_browser_can_go_forward(BrowserHandle) == 0)
                    return false;
                NativeWebView.rnw_browser_go_forward(BrowserHandle);
                return true;
            }

            public unsafe void ExecuteJavaScript(string code)
            {
                CheckClosed();
                var utf8 = MarshalStringToUtf8(code);
                NativeWebView.rnw_browser_execute_js(BrowserHandle, (byte*)utf8);
                Marshal.FreeHGlobal(utf8);
            }

            public void AddResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
                // TODO: implement for window browsers if needed
            }

            public void RemoveResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
            }

            public void Dispose()
            {
                if (Closed) return;

                NativeWebView.rnw_window_close(BrowserHandle);
                Closed = true;

                if (GcHandle.IsAllocated)
                    GcHandle.Free();
            }

            public bool Closed { get; private set; }

            public void OnClose()
            {
                Closed = true;
                _manager._browserWindows.Remove(this);
                _manager._sawmill.Debug("Removing window");
            }

            private void CheckClosed()
            {
                if (Closed)
                    throw new ObjectDisposedException("BrowserWindow");
            }
        }
    }
}
