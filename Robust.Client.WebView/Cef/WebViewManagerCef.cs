using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
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

        public unsafe void Initialize()
        {
            _sawmill = _logManager.GetSawmill("web.cef");

            _consoleHost.RegisterCommand(
                "flushcookies",
                _localization.GetString("cmd-flushcookies-desc"),
                _localization.GetString("cmd-flushcookies-help"),
                (_, _, _) => NativeWebView.rnw_flush_cookies());

#if MACOS
            var subProcessPath = Path.Combine(BasePath, "cef-helper");
            _sawmill.Debug($"Subprocess path: {subProcessPath}");
#else
            string subProcessName;
            if (OperatingSystem.IsWindows())
                subProcessName = "Robust.Client.WebView.exe";
            else if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
                subProcessName = "Robust.Client.WebView";
            else
                throw new NotSupportedException("Unsupported platform for CEF!");

            var subProcessPath = Path.Combine(BasePath, subProcessName);
            var cefResourcesPath = LocateCefResources();
            _sawmill.Debug($"Subprocess path: {subProcessPath}, resources: {cefResourcesPath}");

            if (cefResourcesPath == null)
                throw new InvalidOperationException("Unable to locate cef_resources directory!");
#endif

            var remoteDebugPort = _cfg.GetCVar(WCVars.WebRemoteDebugPort);
            var cachePath = FindAndLockCacheDirectory();
            var userAgentOverride = _cfg.GetCVar(WCVars.WebUserAgentOverride);

            var settings = new RnwSettings
            {
                NoSandbox = 1,
                RemoteDebuggingPort = remoteDebugPort,
            };

            var subprocessPathUtf8 = MarshalStringToUtf8(subProcessPath);
            settings.SubprocessPath = (byte*)subprocessPathUtf8;

#if !MACOS
            var localesDirUtf8 = MarshalStringToUtf8(Path.Combine(cefResourcesPath, "locales"));
            var resourcesDirUtf8 = MarshalStringToUtf8(cefResourcesPath);
            settings.ResourcesDirPath = (byte*)resourcesDirUtf8;
            settings.LocalesDirPath = (byte*)localesDirUtf8;
#endif

            var cachePathUtf8 = MarshalStringToUtf8(cachePath);
            settings.CachePath = (byte*)cachePathUtf8;

            var cookieSchemesUtf8 = MarshalStringToUtf8("usr,res");
            settings.CookieableSchemes = (byte*)cookieSchemesUtf8;

            IntPtr userAgentUtf8 = IntPtr.Zero;
            if (!string.IsNullOrEmpty(userAgentOverride))
            {
                userAgentUtf8 = MarshalStringToUtf8(userAgentOverride);
                settings.UserAgent = (byte*)userAgentUtf8;
            }

#if MACOS
            var frameworkDirPath = PathHelpers.ExecutableRelativeFile(
                "../Frameworks/Chromium Embedded Framework.framework");
            var frameworkPath = Path.Combine(frameworkDirPath, "Chromium Embedded Framework");
            var mainBundlePath = PathHelpers.ExecutableRelativeFile("../..");

            var frameworkPathUtf8 = MarshalStringToUtf8(frameworkPath);
            var frameworkDirPathUtf8 = MarshalStringToUtf8(frameworkDirPath);
            var mainBundlePathUtf8 = MarshalStringToUtf8(mainBundlePath);

            settings.FrameworkPath = (byte*)frameworkPathUtf8;
            settings.FrameworkDirPath = (byte*)frameworkDirPathUtf8;
            settings.MainBundlePath = (byte*)mainBundlePathUtf8;
#endif

            var result = NativeWebView.rnw_initialize(&settings);
            _sawmill.Info($"CEF initialized via cef-rs, result: {result}");

            // Free marshalled strings
            Marshal.FreeHGlobal(subprocessPathUtf8);
#if MACOS
            Marshal.FreeHGlobal(frameworkPathUtf8);
            Marshal.FreeHGlobal(frameworkDirPathUtf8);
            Marshal.FreeHGlobal(mainBundlePathUtf8);
#else
            Marshal.FreeHGlobal(resourcesDirUtf8);
            Marshal.FreeHGlobal(localesDirUtf8);
#endif
            Marshal.FreeHGlobal(cachePathUtf8);
            Marshal.FreeHGlobal(cookieSchemesUtf8);
            if (userAgentUtf8 != IntPtr.Zero)
                Marshal.FreeHGlobal(userAgentUtf8);

            // Register res:// scheme handler if enabled.
            if (_cfg.GetCVar(WCVars.WebResProtocol))
            {
                RegisterSchemeHandler("res", "");
            }

            // Register http://127.0.0.1 handler for local resource serving.
            RegisterSchemeHandler("http", "127.0.0.1");
        }

        private unsafe void RegisterSchemeHandler(string scheme, string domain)
        {
            var schemeUtf8 = MarshalStringToUtf8(scheme);
            var domainUtf8 = MarshalStringToUtf8(domain);
            NativeWebView.rnw_register_scheme_handler(
                (byte*)schemeUtf8,
                (byte*)domainUtf8,
                &SchemeHandlerCallback,
                null);
            Marshal.FreeHGlobal(schemeUtf8);
            Marshal.FreeHGlobal(domainUtf8);
        }

        [UnmanagedCallersOnly]
        private static unsafe int SchemeHandlerCallback(void* userData, ulong requestId, byte* urlPtr, byte* methodPtr)
        {
            try
            {
                var url = Marshal.PtrToStringUTF8((IntPtr)urlPtr) ?? "";
                var method = Marshal.PtrToStringUTF8((IntPtr)methodPtr) ?? "GET";
                var uri = new Uri(url);

                var instance = IoCManager.Resolve<IWebViewManagerImpl>() as WebViewManagerCef;
                if (instance == null)
                    return 0;

                // For res://127.0.0.1/... URLs (rewritten from http://127.0.0.1/... by on_before_browse),
                // dispatch to per-control resource request handlers with the original http:// URL.
                if (uri.Scheme == "res" && uri.Host == "127.0.0.1")
                {
                    var originalUrl = "http://127.0.0.1" + uri.PathAndQuery;
                    foreach (var control in instance._activeControls)
                    {
                        if (control.TryHandleResourceRequest(requestId, originalUrl, method))
                            return 1;
                    }
                    return 0;
                }

                // Try per-control handlers for other URLs.
                foreach (var control in instance._activeControls)
                {
                    if (control.TryHandleResourceRequest(requestId, url, method))
                        return 1;
                }

                // Fall back to res:// resource handling.
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
                                var mimeUtf8 = MarshalStringToUtf8(mime);
                                NativeWebView.rnw_request_set_response(requestId, 200, (byte*)mimeUtf8, dataPtr, data.Length);
                                Marshal.FreeHGlobal(mimeUtf8);
                            }
                        }

                        return 1;
                    }

                    // Not found
                    var notFoundBytes = Encoding.UTF8.GetBytes("Not found");
                    fixed (byte* notFoundPtr = notFoundBytes)
                    {
                        var mimeUtf8 = MarshalStringToUtf8("text/plain");
                        NativeWebView.rnw_request_set_response(requestId, 404, (byte*)mimeUtf8, notFoundPtr, notFoundBytes.Length);
                        Marshal.FreeHGlobal(mimeUtf8);
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

        private static string? LocateCefResources()
        {
            if (ProbeDir(BasePath, out var path))
                return path;

            foreach (var searchDir in NativeDllSearchDirectories())
            {
                if (ProbeDir(searchDir, out path))
                    return path;
            }

            return null;

            static bool ProbeDir(string dir, out string path)
            {
                path = Path.Combine(dir, "cef_resources");
                return Directory.Exists(path);
            }
        }

        internal static string[] NativeDllSearchDirectories()
        {
            var sepChar = OperatingSystem.IsWindows() ? ';' : ':';

            var searchDirectories = ((string)AppContext.GetData("NATIVE_DLL_SEARCH_DIRECTORIES")!)
                .Split(sepChar, StringSplitOptions.RemoveEmptyEntries);

            return searchDirectories;
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

        /// <summary>
        /// Marshal a .NET string to a null-terminated UTF-8 byte buffer allocated with Marshal.AllocHGlobal.
        /// Caller must free with Marshal.FreeHGlobal.
        /// </summary>
        internal static IntPtr MarshalStringToUtf8(string s)
        {
            var bytes = Encoding.UTF8.GetBytes(s);
            var ptr = Marshal.AllocHGlobal(bytes.Length + 1);
            Marshal.Copy(bytes, 0, ptr, bytes.Length);
            Marshal.WriteByte(ptr, bytes.Length, 0); // null terminator
            return ptr;
        }
    }
}
