using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using Robust.Client.Console;
using Robust.Shared.Configuration;
using Robust.Shared.ContentPack;
using Robust.Shared.IoC;
using Robust.Shared.Localization;
using Robust.Shared.Log;
using Robust.Shared.Prototypes;
using Robust.Shared.Utility;

namespace Robust.Client.WebView.Cef
{
    internal sealed partial class WebViewManagerCef : IWebViewManagerImpl
    {
        private static readonly string BasePath = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location!)!;

        [Dependency] private readonly IDependencyCollection _dependencyCollection = default!;
        [Dependency] private readonly IPrototypeManager _prototypeManager = default!;
        [Dependency] private readonly IGameControllerInternal _gameController = default!;
        [Dependency] private readonly IResourceManagerInternal _resourceManager = default!;
        [Dependency] private readonly IClientConsoleHost _consoleHost = default!;
        [Dependency] private readonly IConfigurationManager _cfg = default!;
        [Dependency] private readonly ILogManager _logManager = default!;
        [Dependency] private readonly ILocalizationManager _localization = default!;

        private ISawmill _sawmill = default!;

        // Static reference for use from CEF thread callbacks where IoC is unavailable.
        private static WebViewManagerCef? _instance;

        public unsafe void Initialize()
        {
            _sawmill = _logManager.GetSawmill("web.cef");
            _instance = this;

            _consoleHost.RegisterCommand(
                "flushcookies",
                _localization.GetString("cmd-flushcookies-desc"),
                _localization.GetString("cmd-flushcookies-help"),
                (_, _, _) => NativeWebView.rnw_flush_cookies());

            var subProcessName = OperatingSystem.IsWindows() ? "cef-helper.exe" : "cef-helper";
            var subProcessPath = Path.Combine(BasePath, subProcessName);
            _sawmill.Debug($"Subprocess path: {subProcessPath}");

            var userAgentOverride = _cfg.GetCVar(WCVars.WebUserAgentOverride);

            using var builder = new RnwSettingsBuilder
            {
                NoSandbox = 1,
                RemoteDebuggingPort = _cfg.GetCVar(WCVars.WebRemoteDebugPort),
                SubprocessPath = subProcessPath,
                CachePath = FindAndLockCacheDirectory(),
                CookieableSchemes = "usr,res",
                UserAgent = string.IsNullOrEmpty(userAgentOverride) ? null : userAgentOverride,
#if !MACOS
                ResourcesDirPath = BasePath,
                LocalesDirPath = Path.Combine(BasePath, "locales"),
#else
                FrameworkDirPath = PathHelpers.ExecutableRelativeFile(
                    "../Frameworks/Chromium Embedded Framework.framework"),
                FrameworkPath = Path.Combine(
                    PathHelpers.ExecutableRelativeFile("../Frameworks/Chromium Embedded Framework.framework"),
                    "Chromium Embedded Framework"),
                MainBundlePath = PathHelpers.ExecutableRelativeFile("../.."),
#endif
            };

            var result = NativeWebView.rnw_initialize(&builder.Settings);
            _sawmill.Info($"CEF initialized via cef-rs, result: {result}");

            // Register res:// scheme handler if enabled.
            if (_cfg.GetCVar(WCVars.WebResProtocol))
            {
                RegisterSchemeHandler("res", "");
            }
        }

        private unsafe void RegisterSchemeHandler(string scheme, string domain)
        {
            NativeWebView.rnw_register_scheme_handler(
                scheme,
                domain,
                &SchemeHandlerCallback,
                null);
        }

        [UnmanagedCallersOnly]
        private static unsafe int SchemeHandlerCallback(
            void* userData, byte* urlPtr, byte* methodPtr, ResponseContext* responseCtx)
        {
            try
            {
                var url = Marshal.PtrToStringUTF8((IntPtr)urlPtr) ?? "";
                var uri = new Uri(url);

                var instance = _instance;
                if (instance == null)
                    return 0;

                // Handle res:// resource loading.
                if (uri.Scheme == "res")
                {
                    var resPath = new ResPath(uri.AbsolutePath);

                    if (instance._resourceManager.TryContentFileRead(resPath, out var stream))
                    {
                        if (!instance.TryGetResourceMimeType(resPath.Extension, out var mime))
                            mime = "application/octet-stream";

                        using (stream)
                        {
                            using var ms = new MemoryStream();
                            stream.CopyTo(ms);
                            var data = ms.ToArray();
                            fixed (byte* dataPtr = data)
                            {
                                NativeWebView.rnw_response_write(responseCtx, 200, mime, dataPtr, data.Length);
                            }
                        }

                        return 1;
                    }

                    // Not found
                    var notFoundBytes = Encoding.UTF8.GetBytes("Not found");
                    fixed (byte* notFoundPtr = notFoundBytes)
                    {
                        NativeWebView.rnw_response_write(responseCtx, 404, "text/plain", notFoundPtr, notFoundBytes.Length);
                    }

                    return 1;
                }

                return 0;
            }
            catch (Exception ex)
            {
                System.Console.Error.WriteLine($"[rnw] SchemeHandlerCallback exception: {ex}");
                return 0;
            }
        }

        public void Update()
        {
            NativeWebView.rnw_do_message_loop_work();
        }

        public void Shutdown()
        {
            foreach (var control in _activeControls.ToArray())
            {
                control.CloseBrowser();
            }

            foreach (var window in _browserWindows.ToArray())
            {
                window.Dispose();
            }

            NativeWebView.rnw_shutdown();
        }
    }
}
