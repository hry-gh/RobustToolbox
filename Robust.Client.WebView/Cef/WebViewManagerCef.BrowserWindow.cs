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
            };

            var handle = NativeWebView.rnw_window_create(
                createParams.Url,
                createParams.Width,
                createParams.Height,
                &callbacks);

            impl.BrowserHandle = handle;
            impl.GcHandle = gcHandle;
            _browserWindows.Add(impl);

            return impl;
        }

        [UnmanagedCallersOnly]
        private static unsafe int WindowOnBeforeBrowseCallback(
            void* userData, byte* urlPtr, int userGesture, int isRedirect)
        {
            return 0;
        }

        [UnmanagedCallersOnly]
        private static unsafe int WindowOnResourceRequestCallback(
            void* userData, ulong requestId, byte* urlPtr, byte* methodPtr)
        {
            var impl = ResolveWindow(userData);
            if (impl == null) return 0;

            var url = Marshal.PtrToStringUTF8((IntPtr)urlPtr) ?? "";
            var method = Marshal.PtrToStringUTF8((IntPtr)methodPtr) ?? "GET";

            var context = new NativeRequestHandlerContext(url, method);

            lock (impl._resourceRequestHandlers)
            {
                foreach (var handler in impl._resourceRequestHandlers)
                {
                    handler(context);

                    if (context.IsCancelled)
                        return 0;

                    if (context.ResponseData != null)
                    {
                        var data = context.ResponseData;
                        NativeWebView.rnw_request_set_response(
                            requestId,
                            data.StatusCode,
                            data.MimeType,
                            data.Data);
                        return 1;
                    }
                }
            }

            return 0;
        }

        [UnmanagedCallersOnly]
        private static unsafe void WindowOnBeforeCloseCallback(void* userData)
        {
            var impl = ResolveWindow(userData);
            impl?.OnClose();
        }

        private static unsafe WebViewWindowImpl? ResolveWindow(void* userData)
        {
            var handle = GCHandle.FromIntPtr((IntPtr)userData);
            return handle.Target as WebViewWindowImpl;
        }

        private sealed class WebViewWindowImpl : IWebViewWindow
        {
            private readonly WebViewManagerCef _manager;
            internal ulong BrowserHandle;
            internal GCHandle GcHandle;
            internal readonly List<Action<IRequestHandlerContext>> _resourceRequestHandlers = new();

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
                    NativeWebView.rnw_browser_load_url(BrowserHandle, value);
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

            public void ExecuteJavaScript(string code)
            {
                CheckClosed();
                NativeWebView.rnw_browser_execute_js(BrowserHandle, code);
            }

            public void AddResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
                lock (_resourceRequestHandlers)
                {
                    _resourceRequestHandlers.Add(handler);
                }
            }

            public void RemoveResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
                lock (_resourceRequestHandlers)
                {
                    _resourceRequestHandlers.Remove(handler);
                }
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
