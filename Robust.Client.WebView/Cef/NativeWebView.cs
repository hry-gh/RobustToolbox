using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Robust.Client.WebView.Cef;

/// <summary>
/// P/Invoke declarations for the robust_native_webview Rust cdylib.
/// </summary>
internal static unsafe partial class NativeWebView
{
    private const string LibName = "robust_native_webview";

    // ========================================================================
    // Lifecycle
    // ========================================================================

    [LibraryImport(LibName)]
    internal static partial int rnw_initialize(RnwSettings* settings);

    [LibraryImport(LibName)]
    internal static partial void rnw_do_message_loop_work();

    [LibraryImport(LibName)]
    internal static partial void rnw_shutdown();

    [LibraryImport(LibName)]
    internal static partial void rnw_flush_cookies();

    // ========================================================================
    // Browser Management
    // ========================================================================

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial ulong rnw_browser_create(
        string url,
        int width,
        int height,
        RnwBrowserCallbacks* callbacks);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_close(ulong handle);

    [LibraryImport(LibName)]
    internal static partial int rnw_browser_get_url(ulong handle, byte* buf, int bufLen);

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void rnw_browser_load_url(ulong handle, string url);

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void rnw_browser_execute_js(ulong handle, string code);

    [LibraryImport(LibName)]
    internal static partial int rnw_browser_is_loading(ulong handle);

    [LibraryImport(LibName)]
    internal static partial int rnw_browser_can_go_back(ulong handle);

    [LibraryImport(LibName)]
    internal static partial int rnw_browser_can_go_forward(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_go_back(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_go_forward(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_reload(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_stop_load(ulong handle);

    // ========================================================================
    // Input
    // ========================================================================

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_send_mouse_move(
        ulong handle, int x, int y, uint modifiers, int mouseLeave);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_send_mouse_click(
        ulong handle, int x, int y, uint modifiers, int button, int mouseUp, int clickCount);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_send_mouse_wheel(
        ulong handle, int x, int y, uint modifiers, int deltaX, int deltaY);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_send_key_event(ulong handle, RnwKeyEvent* keyEvent);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_was_resized(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_invalidate(ulong handle);

    [LibraryImport(LibName)]
    internal static partial void rnw_browser_notify_move_or_resize_started(ulong handle);

    // ========================================================================
    // Resource Request Response
    // ========================================================================

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void rnw_request_set_response(
        int statusCode, string mimeType, byte* data, int dataLen);

    // ========================================================================
    // Scheme Handler Registration
    // ========================================================================

    [LibraryImport(LibName)]
    internal static partial void rnw_register_res_scheme_handler(
        delegate* unmanaged<void*, byte*, byte*, int> callback,
        void* userData);

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void rnw_register_scheme_handler(
        string scheme,
        string domain,
        delegate* unmanaged<void*, byte*, byte*, int> callback,
        void* userData);

    // ========================================================================
    // Window Browser
    // ========================================================================

    [LibraryImport(LibName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial ulong rnw_window_create(
        string url,
        int width,
        int height,
        RnwBrowserCallbacks* callbacks);

    [LibraryImport(LibName)]
    internal static partial void rnw_window_close(ulong handle);
}

// ============================================================================
// FFI Structs
// ============================================================================

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct RnwSettings
{
    public int NoSandbox;
    public byte* SubprocessPath;
    public byte* ResourcesDirPath;
    public byte* LocalesDirPath;
    public int RemoteDebuggingPort;
    public byte* CachePath;
    public byte* UserAgent;
    public byte* CookieableSchemes;
    /// macOS only: path to the Chromium Embedded Framework dylib.
    public byte* FrameworkPath;
    /// macOS only: path to the .framework directory.
    public byte* FrameworkDirPath;
    /// macOS only: path to the main app bundle.
    public byte* MainBundlePath;
}

/// <summary>
/// Disposable builder for RnwSettings that manages string lifetime.
/// All strings are freed when the builder is disposed.
/// </summary>
internal unsafe struct RnwSettingsBuilder : IDisposable
{
    public RnwSettings Settings;
    private List<IntPtr>? _allocations;

    public int NoSandbox { set => Settings.NoSandbox = value; }
    public int RemoteDebuggingPort { set => Settings.RemoteDebuggingPort = value; }

    public string? SubprocessPath { set => Settings.SubprocessPath = AllocUtf8(value); }
    public string? ResourcesDirPath { set => Settings.ResourcesDirPath = AllocUtf8(value); }
    public string? LocalesDirPath { set => Settings.LocalesDirPath = AllocUtf8(value); }
    public string? CachePath { set => Settings.CachePath = AllocUtf8(value); }
    public string? UserAgent { set => Settings.UserAgent = AllocUtf8(value); }
    public string? CookieableSchemes { set => Settings.CookieableSchemes = AllocUtf8(value); }
    public string? FrameworkPath { set => Settings.FrameworkPath = AllocUtf8(value); }
    public string? FrameworkDirPath { set => Settings.FrameworkDirPath = AllocUtf8(value); }
    public string? MainBundlePath { set => Settings.MainBundlePath = AllocUtf8(value); }

    private byte* AllocUtf8(string? s)
    {
        if (s == null)
            return null;

        var bytes = System.Text.Encoding.UTF8.GetBytes(s);
        var ptr = Marshal.AllocHGlobal(bytes.Length + 1);
        Marshal.Copy(bytes, 0, ptr, bytes.Length);
        Marshal.WriteByte(ptr, bytes.Length, 0);

        _allocations ??= [];
        _allocations.Add(ptr);

        return (byte*)ptr;
    }

    public void Dispose()
    {
        if (_allocations == null)
            return;

        foreach (var ptr in _allocations)
            Marshal.FreeHGlobal(ptr);

        _allocations = null;
    }
}

[StructLayout(LayoutKind.Sequential)]
internal struct RnwKeyEvent
{
    /// <summary>0 = RawKeyDown, 1 = KeyUp, 2 = Char</summary>
    public int EventType;
    public int WindowsKeyCode;
    public int NativeKeyCode;
    public ushort Character;
    public ushort UnmodifiedCharacter;
    public uint Modifiers;
    public int IsSystemKey;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct RnwBrowserCallbacks
{
    public void* UserData;

    public delegate* unmanaged<void*, int, int, byte*, int, int*, void> OnPaint;
    public delegate* unmanaged<void*, int*, int*, void> GetViewRect;
    public delegate* unmanaged<void*, float*, void> GetScreenInfo;
    public delegate* unmanaged<void*, int, void> OnVirtualKeyboardRequested;
    public delegate* unmanaged<void*, byte*, int, int, int> OnBeforeBrowse;
    public delegate* unmanaged<void*, byte*, byte*, int> OnResourceRequest;
    public delegate* unmanaged<void*, void> OnLoadStart;
    public delegate* unmanaged<void*, int, void> OnLoadEnd;
    public delegate* unmanaged<void*, void> OnBeforeClose;
}
