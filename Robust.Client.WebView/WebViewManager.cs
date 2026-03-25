using System;
using System.Diagnostics.CodeAnalysis;
using System.Runtime.InteropServices;
using Robust.Client.WebView;
#if ROBUST_CEF
using Robust.Client.WebView.Cef;
#endif
using Robust.Client.WebView.Headless;
using Robust.Client.WebView.Native;
using Robust.Client.WebViewHook;
using Robust.Shared.Configuration;
using Robust.Shared.IoC;
using Robust.Shared.Log;
using Robust.Shared.Reflection;
using Robust.Shared.Utility;

[assembly: WebViewManagerImpl(typeof(WebViewManager))]

namespace Robust.Client.WebView
{
    internal sealed class WebViewManager : IWebViewManagerInternal, IWebViewManagerHook
    {
        private IWebViewManagerImpl? _impl;

        public void PreInitialize(IDependencyCollection dependencies, GameController.DisplayMode mode)
        {
            DebugTools.Assert(_impl == null, "WebViewManager has already been initialized!");

            var cfg = dependencies.Resolve<IConfigurationManagerInternal>();
            cfg.LoadCVarsFromAssembly(typeof(WebViewManager).Assembly);

            var refl = dependencies.Resolve<IReflectionManager>();
            refl.LoadAssemblies(typeof(WebViewManager).Assembly);

            dependencies.RegisterInstance<IWebViewManager>(this);
            dependencies.RegisterInstance<IWebViewManagerInternal>(this);

            if (mode == GameController.DisplayMode.Headless || cfg.GetCVar(WCVars.WebHeadless))
            {
                _impl = new WebViewManagerHeadless();
            }
            else
            {
                _impl = CreateBackendImpl(cfg, dependencies.Resolve<ILogManager>());
            }

            dependencies.InjectDependencies(_impl, oneOff: true);
        }

        private static IWebViewManagerImpl CreateBackendImpl(IConfigurationManager cfg, ILogManager logManager)
        {
            var backend = cfg.GetCVar(WCVars.WebBackend);
            var sawmill = logManager.GetSawmill("web");

            switch (backend.ToLowerInvariant())
            {
                case "native":
                    if (TryCreateNativeBackend(out var native))
                    {
                        sawmill.Info("Using native webview backend");
                        return native;
                    }
                    sawmill.Warning("Native webview backend unavailable, falling back");
                    return CreateCefOrHeadless(sawmill);

                case "auto":
                    if (TryCreateNativeBackend(out native))
                    {
                        sawmill.Info("Using native webview backend (auto-selected)");
                        return native;
                    }
                    return CreateCefOrHeadless(sawmill);

                case "cef":
                default:
                    return CreateCefOrHeadless(sawmill);
            }
        }

        private static IWebViewManagerImpl CreateCefOrHeadless(ISawmill sawmill)
        {
#if ROBUST_CEF
            sawmill.Info("Using CEF webview backend");
            return new WebViewManagerCef();
#else
            sawmill.Warning("CEF not available, using headless webview");
            return new WebViewManagerHeadless();
#endif
        }

        private static bool TryCreateNativeBackend([NotNullWhen(true)] out IWebViewManagerImpl? impl)
        {
            impl = null;

            try
            {
                var result = WebViewNative.robust_webview_init();
                if (result == 0)
                {
                    WebViewNative.robust_webview_shutdown();
                    impl = new WebViewManagerNative();
                    return true;
                }
            }
            catch (DllNotFoundException)
            {
                // deliberately empty
            }
            catch (EntryPointNotFoundException)
            {
                // deliberately empty
            }

            return false;
        }

        public void Initialize()
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            _impl!.Initialize();
        }

        public void Update()
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            _impl!.Update();
        }

        public void Shutdown()
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            _impl!.Shutdown();
        }

        public IWebViewWindow CreateBrowserWindow(BrowserWindowCreateParameters createParams)
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            return _impl!.CreateBrowserWindow(createParams);
        }

        public void SetResourceMimeType(string extension, string mimeType)
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            _impl!.SetResourceMimeType(extension, mimeType);
        }

        public bool TryGetResourceMimeType(string extension, [NotNullWhen(true)] out string? mimeType)
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            return _impl!.TryGetResourceMimeType(extension, out mimeType);
        }

        public IWebViewControlImpl MakeControlImpl(WebViewControl owner)
        {
            DebugTools.Assert(_impl != null, "WebViewManager has not yet been initialized!");

            return _impl!.MakeControlImpl(owner);
        }
    }
}
