using System;
using System.Runtime.InteropServices;

namespace Robust.Client.WebView.Native;

internal static partial class WebViewNative
{
    private const string Lib = "robust_native_webview";

    [DllImport(Lib)]
    internal static extern int robust_webview_init();

    [DllImport(Lib)]
    internal static extern void robust_webview_shutdown();

    [DllImport(Lib)]
    internal static extern void robust_webview_pump();

    [DllImport(Lib, CharSet = CharSet.Ansi)]
    internal static extern nint robust_webview_create(nint parent, string? url);

    [DllImport(Lib)]
    internal static extern void robust_webview_destroy(nint handle);

    [DllImport(Lib, CharSet = CharSet.Ansi)]
    internal static extern void robust_webview_navigate(nint handle, string url);

    [DllImport(Lib)]
    internal static extern void robust_webview_reload(nint handle);

    [DllImport(Lib)]
    internal static extern void robust_webview_stop(nint handle);

    [DllImport(Lib)]
    internal static extern void robust_webview_go_back(nint handle);

    [DllImport(Lib)]
    internal static extern void robust_webview_go_forward(nint handle);

    [DllImport(Lib)]
    [return: MarshalAs(UnmanagedType.U1)]
    internal static extern bool robust_webview_can_go_back(nint handle);

    [DllImport(Lib)]
    [return: MarshalAs(UnmanagedType.U1)]
    internal static extern bool robust_webview_can_go_forward(nint handle);

    [DllImport(Lib)]
    [return: MarshalAs(UnmanagedType.U1)]
    internal static extern bool robust_webview_is_loading(nint handle);

    [DllImport(Lib, CharSet = CharSet.Ansi)]
    internal static extern void robust_webview_execute_js(nint handle, string code);

    [DllImport(Lib)]
    internal static extern void robust_webview_set_size(nint handle, int width, int height);

    [DllImport(Lib)]
    internal static extern void robust_webview_set_bounds(nint handle, int x, int y, int width, int height);

    [DllImport(Lib, CharSet = CharSet.Ansi)]
    internal static extern void robust_webview_load_html(nint handle, string html, string? baseUrl);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal unsafe delegate void SchemeCallback(byte* url, nint requestHandle, nint userData);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal unsafe delegate void MessageCallback(nint handle, byte* message, nint userData);

    [DllImport(Lib)]
    internal static extern void robust_webview_set_scheme_handler(SchemeCallback? callback, nint userData);

    [DllImport(Lib)]
    internal static extern unsafe void robust_webview_respond_scheme(
        nint requestHandle,
        byte* data,
        int length,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string mimeType,
        int statusCode);

    [DllImport(Lib)]
    internal static extern void robust_webview_set_message_handler(
        nint handle,
        MessageCallback? callback,
        nint userData);
}
