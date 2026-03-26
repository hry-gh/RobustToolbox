using System;
using System.Collections.Generic;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Text;
using Robust.Client.Graphics;
using Robust.Client.Input;
using Robust.Client.UserInterface;
using Robust.Shared.IoC;
using Robust.Shared.Log;
using Robust.Shared.Maths;
using Robust.Shared.Utility;
using SixLabors.ImageSharp.PixelFormats;
using static Robust.Client.WebView.Cef.CefKeyCodes.ChromiumKeyboardCode;
using static Robust.Client.Input.Keyboard;

namespace Robust.Client.WebView.Cef
{
    internal partial class WebViewManagerCef
    {
        private readonly List<ControlImpl> _activeControls = new();

        public IWebViewControlImpl MakeControlImpl(WebViewControl owner)
        {
            var shader = _prototypeManager.Index<ShaderPrototype>("bgra");
            var shaderInstance = shader.Instance();
            var impl = new ControlImpl(this, owner, shaderInstance);
            _dependencyCollection.InjectDependencies(impl);
            return impl;
        }

        private sealed class ControlImpl : IWebViewControlImpl
        {
            private static readonly Dictionary<Key, CefKeyCodes.ChromiumKeyboardCode> KeyMap = new()
            {
                [Key.A] = VKEY_A,
                [Key.B] = VKEY_B,
                [Key.C] = VKEY_C,
                [Key.D] = VKEY_D,
                [Key.E] = VKEY_E,
                [Key.F] = VKEY_F,
                [Key.G] = VKEY_G,
                [Key.H] = VKEY_H,
                [Key.I] = VKEY_I,
                [Key.J] = VKEY_J,
                [Key.K] = VKEY_K,
                [Key.L] = VKEY_L,
                [Key.M] = VKEY_M,
                [Key.N] = VKEY_N,
                [Key.O] = VKEY_O,
                [Key.P] = VKEY_P,
                [Key.Q] = VKEY_Q,
                [Key.R] = VKEY_R,
                [Key.S] = VKEY_S,
                [Key.T] = VKEY_T,
                [Key.U] = VKEY_U,
                [Key.V] = VKEY_V,
                [Key.W] = VKEY_W,
                [Key.X] = VKEY_X,
                [Key.Y] = VKEY_Y,
                [Key.Z] = VKEY_Z,
                [Key.Num0] = VKEY_0,
                [Key.Num1] = VKEY_1,
                [Key.Num2] = VKEY_2,
                [Key.Num3] = VKEY_3,
                [Key.Num4] = VKEY_4,
                [Key.Num5] = VKEY_5,
                [Key.Num6] = VKEY_6,
                [Key.Num7] = VKEY_7,
                [Key.Num8] = VKEY_8,
                [Key.Num9] = VKEY_9,
                [Key.NumpadNum0] = VKEY_NUMPAD0,
                [Key.NumpadNum1] = VKEY_NUMPAD1,
                [Key.NumpadNum2] = VKEY_NUMPAD2,
                [Key.NumpadNum3] = VKEY_NUMPAD3,
                [Key.NumpadNum4] = VKEY_NUMPAD4,
                [Key.NumpadNum5] = VKEY_NUMPAD5,
                [Key.NumpadNum6] = VKEY_NUMPAD6,
                [Key.NumpadNum7] = VKEY_NUMPAD7,
                [Key.NumpadNum8] = VKEY_NUMPAD8,
                [Key.NumpadNum9] = VKEY_NUMPAD9,
                [Key.Escape] = VKEY_ESCAPE,
                [Key.Control] = VKEY_CONTROL,
                [Key.Shift] = VKEY_SHIFT,
                [Key.Alt] = VKEY_MENU,
                [Key.LSystem] = VKEY_LWIN,
                [Key.RSystem] = VKEY_RWIN,
                [Key.LBracket] = VKEY_OEM_4,
                [Key.RBracket] = VKEY_OEM_6,
                [Key.SemiColon] = VKEY_OEM_1,
                [Key.Comma] = VKEY_OEM_COMMA,
                [Key.Period] = VKEY_OEM_PERIOD,
                [Key.Apostrophe] = VKEY_OEM_7,
                [Key.Slash] = VKEY_OEM_2,
                [Key.BackSlash] = VKEY_OEM_5,
                [Key.Tilde] = VKEY_OEM_3,
                [Key.Equal] = VKEY_OEM_PLUS,
                [Key.Space] = VKEY_SPACE,
                [Key.Return] = VKEY_RETURN,
                [Key.BackSpace] = VKEY_BACK,
                [Key.Tab] = VKEY_TAB,
                [Key.PageUp] = VKEY_PRIOR,
                [Key.PageDown] = VKEY_NEXT,
                [Key.End] = VKEY_END,
                [Key.Home] = VKEY_HOME,
                [Key.Insert] = VKEY_INSERT,
                [Key.Delete] = VKEY_DELETE,
                [Key.Minus] = VKEY_OEM_MINUS,
                [Key.NumpadAdd] = VKEY_ADD,
                [Key.NumpadSubtract] = VKEY_SUBTRACT,
                [Key.NumpadDivide] = VKEY_DIVIDE,
                [Key.NumpadMultiply] = VKEY_MULTIPLY,
                [Key.NumpadDecimal] = VKEY_DECIMAL,
                [Key.Left] = VKEY_LEFT,
                [Key.Right] = VKEY_RIGHT,
                [Key.Up] = VKEY_UP,
                [Key.Down] = VKEY_DOWN,
                [Key.F1] = VKEY_F1,
                [Key.F2] = VKEY_F2,
                [Key.F3] = VKEY_F3,
                [Key.F4] = VKEY_F4,
                [Key.F5] = VKEY_F5,
                [Key.F6] = VKEY_F6,
                [Key.F7] = VKEY_F7,
                [Key.F8] = VKEY_F8,
                [Key.F9] = VKEY_F9,
                [Key.F10] = VKEY_F10,
                [Key.F11] = VKEY_F11,
                [Key.F12] = VKEY_F12,
                [Key.F13] = VKEY_F13,
                [Key.F14] = VKEY_F14,
                [Key.F15] = VKEY_F15,
                [Key.Pause] = VKEY_PAUSE,
            };

            // CefEventFlags constants matching CEF's cef_event_flags_t
            private const uint EVENTFLAG_NONE = 0;
            private const uint EVENTFLAG_CONTROL_DOWN = 1 << 3;
            private const uint EVENTFLAG_ALT_DOWN = 1 << 5;
            private const uint EVENTFLAG_SHIFT_DOWN = 1 << 1;
            private const uint EVENTFLAG_LEFT_MOUSE_BUTTON = 1 << 6;
            private const uint EVENTFLAG_MIDDLE_MOUSE_BUTTON = 1 << 7;
            private const uint EVENTFLAG_RIGHT_MOUSE_BUTTON = 1 << 8;

            [Dependency] private readonly IClyde _clyde = default!;
            [Dependency] private readonly IInputManager _inputMgr = default!;

            private readonly WebViewManagerCef _manager;
            public readonly WebViewControl Owner;
            private readonly ShaderInstance _shaderInstance;

            // Handler dispatch lists (no longer using CefGlue types).
            private readonly List<Action<IRequestHandlerContext>> _resourceRequestHandlers = new();
            private readonly List<Action<IBeforeBrowseContext>> _beforeBrowseHandlers = new();

            public ControlImpl(WebViewManagerCef manager, WebViewControl owner, ShaderInstance shaderInstance)
            {
                _manager = manager;
                Owner = owner;
                _shaderInstance = shaderInstance;
            }

            private const int ScrollSpeed = 50;

            private bool _textInputActive;

            private ulong _browserHandle;
            private GCHandle _callbackGcHandle;
            private LiveData? _data;
            private string _startUrl = "about:blank";

            public unsafe string Url
            {
                get
                {
                    if (_browserHandle == 0)
                        return _startUrl;

                    var buf = stackalloc byte[4096];
                    var len = NativeWebView.rnw_browser_get_url(_browserHandle, buf, 4096);
                    if (len < 0)
                        return _startUrl;

                    return Encoding.UTF8.GetString(buf, Math.Min(len, 4095));
                }
                set
                {
                    if (_browserHandle == 0)
                    {
                        _startUrl = value;
                        return;
                    }

                    var utf8 = MarshalStringToUtf8(value);
                    NativeWebView.rnw_browser_load_url(_browserHandle, (byte*)utf8);
                    Marshal.FreeHGlobal(utf8);
                }
            }

            public bool IsOpen => _browserHandle != 0;
            public bool IsLoading => _browserHandle != 0 && NativeWebView.rnw_browser_is_loading(_browserHandle) != 0;

            public unsafe void StartBrowser()
            {
                DebugTools.Assert(_browserHandle == 0);

                // Pin ourselves so the GC doesn't move us while callbacks are active.
                _callbackGcHandle = GCHandle.Alloc(this);

                var callbacks = new RnwBrowserCallbacks
                {
                    UserData = (void*)GCHandle.ToIntPtr(_callbackGcHandle),
                    OnPaint = &OnPaintCallback,
                    GetViewRect = &GetViewRectCallback,
                    GetScreenInfo = &GetScreenInfoCallback,
                    OnVirtualKeyboardRequested = &OnVirtualKeyboardRequestedCallback,
                    OnBeforeBrowse = &OnBeforeBrowseCallback,
                    OnResourceRequest = &OnResourceRequestCallback,
                    OnLoadStart = &OnLoadStartCallback,
                    OnLoadEnd = &OnLoadEndCallback,
                    OnBeforeClose = null,
                };

                var urlUtf8 = MarshalStringToUtf8(_startUrl);
                _browserHandle = NativeWebView.rnw_browser_create(
                    (byte*)urlUtf8,
                    Math.Max(Owner.PixelWidth, 1),
                    Math.Max(Owner.PixelHeight, 1),
                    &callbacks);
                Marshal.FreeHGlobal(urlUtf8);

                var texture = _clyde.CreateBlankTexture<Rgba32>(Vector2i.One);
                _data = new LiveData(texture);

                _manager._activeControls.Add(this);
            }

            public void CloseBrowser()
            {
                DebugTools.Assert(_browserHandle != 0);

                _data!.Texture.Dispose();
                NativeWebView.rnw_browser_close(_browserHandle);
                _browserHandle = 0;
                _data = null;

                if (_callbackGcHandle.IsAllocated)
                    _callbackGcHandle.Free();

                _manager._activeControls.Remove(this);
            }

            public void MouseMove(GUIMouseMoveEventArgs args)
            {
                if (_browserHandle == 0) return;
                var modifiers = CalcMouseModifiers();
                NativeWebView.rnw_browser_send_mouse_move(
                    _browserHandle,
                    (int)args.RelativePosition.X, (int)args.RelativePosition.Y,
                    modifiers, 0);
            }

            public void MouseExited()
            {
                if (_browserHandle == 0) return;
                var modifiers = CalcMouseModifiers();
                NativeWebView.rnw_browser_send_mouse_move(_browserHandle, 0, 0, modifiers, 1);
            }

            public void MouseWheel(GUIMouseWheelEventArgs args)
            {
                if (_browserHandle == 0) return;
                var modifiers = CalcMouseModifiers();
                NativeWebView.rnw_browser_send_mouse_wheel(
                    _browserHandle,
                    (int)args.RelativePosition.X, (int)args.RelativePosition.Y,
                    modifiers,
                    (int)args.Delta.X * ScrollSpeed,
                    (int)args.Delta.Y * ScrollSpeed);
            }

            public unsafe bool RawKeyEvent(in GuiRawKeyEvent guiRawEvent)
            {
                if (_browserHandle == 0)
                    return false;

                if (guiRawEvent.Key is Key.MouseLeft or Key.MouseMiddle or Key.MouseRight)
                {
                    var button = guiRawEvent.Key switch
                    {
                        Key.MouseLeft => 0,   // MBT_LEFT
                        Key.MouseMiddle => 1,  // MBT_MIDDLE
                        Key.MouseRight => 2,   // MBT_RIGHT
                        _ => 0
                    };

                    NativeWebView.rnw_browser_send_mouse_click(
                        _browserHandle,
                        guiRawEvent.MouseRelative.X, guiRawEvent.MouseRelative.Y,
                        EVENTFLAG_NONE,
                        button,
                        guiRawEvent.Action == RawKeyAction.Up ? 1 : 0,
                        1);
                }
                else
                {
                    if (!KeyMap.TryGetValue(guiRawEvent.Key, out var vkKey))
                        vkKey = default;

#if !MACOS
                    var lParam = 0;
                    lParam |= (guiRawEvent.ScanCode & 0xFF) << 16;
                    if (guiRawEvent.Action != RawKeyAction.Down)
                        lParam |= 1 << 30;
                    if (guiRawEvent.Action == RawKeyAction.Up)
                        lParam |= 1 << 31;
#else
                    var lParam = guiRawEvent.RawCode;
#endif
                    var modifiers = CalcModifiers(guiRawEvent.Key);

                    var keyEvent = new RnwKeyEvent
                    {
                        EventType = guiRawEvent.Action == RawKeyAction.Up ? 1 : 0, // 0=RawKeyDown, 1=KeyUp
                        NativeKeyCode = lParam,
                        WindowsKeyCode = (int)vkKey,
                        IsSystemKey = 0,
                        Modifiers = modifiers,
                    };

                    NativeWebView.rnw_browser_send_key_event(_browserHandle, &keyEvent);

                    if (guiRawEvent.Action != RawKeyAction.Up && guiRawEvent.Key == Key.Return)
                    {
                        var charEvent = new RnwKeyEvent
                        {
                            EventType = 2, // Char
                            WindowsKeyCode = '\b',
                            NativeKeyCode = lParam,
                            Modifiers = modifiers,
                        };

                        NativeWebView.rnw_browser_send_key_event(_browserHandle, &charEvent);
                    }
                }

                return true;
            }

            private uint CalcModifiers(Key key)
            {
                uint modifiers = EVENTFLAG_NONE;
                if (_inputMgr.IsKeyDown(Key.Control))
                    modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (_inputMgr.IsKeyDown(Key.Alt))
                    modifiers |= EVENTFLAG_ALT_DOWN;
                if (_inputMgr.IsKeyDown(Key.Shift))
                    modifiers |= EVENTFLAG_SHIFT_DOWN;
                return modifiers;
            }

            private uint CalcMouseModifiers()
            {
                uint modifiers = EVENTFLAG_NONE;
                if (_inputMgr.IsKeyDown(Key.Control))
                    modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (_inputMgr.IsKeyDown(Key.Alt))
                    modifiers |= EVENTFLAG_ALT_DOWN;
                if (_inputMgr.IsKeyDown(Key.Shift))
                    modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (_inputMgr.IsKeyDown(Key.MouseLeft))
                    modifiers |= EVENTFLAG_LEFT_MOUSE_BUTTON;
                if (_inputMgr.IsKeyDown(Key.MouseMiddle))
                    modifiers |= EVENTFLAG_MIDDLE_MOUSE_BUTTON;
                if (_inputMgr.IsKeyDown(Key.MouseRight))
                    modifiers |= EVENTFLAG_RIGHT_MOUSE_BUTTON;
                return modifiers;
            }

            public unsafe void TextEntered(GUITextEnteredEventArgs args)
            {
                if (_browserHandle == 0) return;

                foreach (var chr in args.Text)
                {
                    var charEvent = new RnwKeyEvent
                    {
                        EventType = 2, // Char
                        WindowsKeyCode = chr,
                        Character = chr,
                        UnmodifiedCharacter = chr,
                    };

                    NativeWebView.rnw_browser_send_key_event(_browserHandle, &charEvent);
                }
            }

            public void Resized()
            {
                if (_browserHandle == 0) return;

                NativeWebView.rnw_browser_notify_move_or_resize_started(_browserHandle);
                NativeWebView.rnw_browser_was_resized(_browserHandle);
                _data!.Texture.Dispose();
                _data.Texture = _clyde.CreateBlankTexture<Rgba32>((Owner.PixelWidth, Owner.PixelHeight));
                NativeWebView.rnw_browser_invalidate(_browserHandle);
            }

            public void Draw(DrawingHandleScreen handle)
            {
                if (_data == null) return;

                if (_data.IsDirty)
                {
                    _data.IsDirty = false;
                    var bufImg = _data.Buffer.Buffer;
                    _data.Texture.SetSubImage(
                        Vector2i.Zero,
                        bufImg,
                        new UIBox2i(
                            0, 0,
                            Math.Min(Owner.PixelWidth, bufImg.Width),
                            Math.Min(Owner.PixelHeight, bufImg.Height)));
                }

                handle.UseShader(_shaderInstance);
                handle.DrawTexture(_data.Texture, Vector2.Zero);
            }

            public unsafe void StopLoad()
            {
                if (_browserHandle == 0) throw new InvalidOperationException();
                NativeWebView.rnw_browser_stop_load(_browserHandle);
            }

            public void Reload()
            {
                if (_browserHandle == 0) throw new InvalidOperationException();
                NativeWebView.rnw_browser_reload(_browserHandle);
            }

            public bool GoBack()
            {
                if (_browserHandle == 0) throw new InvalidOperationException();
                if (NativeWebView.rnw_browser_can_go_back(_browserHandle) == 0)
                    return false;
                NativeWebView.rnw_browser_go_back(_browserHandle);
                return true;
            }

            public bool GoForward()
            {
                if (_browserHandle == 0) throw new InvalidOperationException();
                if (NativeWebView.rnw_browser_can_go_forward(_browserHandle) == 0)
                    return false;
                NativeWebView.rnw_browser_go_forward(_browserHandle);
                return true;
            }

            public unsafe void ExecuteJavaScript(string code)
            {
                if (_browserHandle == 0) throw new InvalidOperationException();
                var utf8 = MarshalStringToUtf8(code);
                NativeWebView.rnw_browser_execute_js(_browserHandle, (byte*)utf8);
                Marshal.FreeHGlobal(utf8);
            }

            /// <summary>
            /// Try to handle a resource request via the per-control handlers.
            /// Called from the global scheme handler callback.
            /// </summary>
            internal unsafe bool TryHandleResourceRequest(ulong requestId, string url, string method)
            {
                var context = new NativeRequestHandlerContext(url, method);

                lock (_resourceRequestHandlers)
                {
                    foreach (var handler in _resourceRequestHandlers)
                    {
                        handler(context);

                        if (context.IsCancelled)
                            return false;

                        if (context.ResponseData != null)
                        {
                            var data = context.ResponseData;
                            var mimeUtf8 = MarshalStringToUtf8(data.MimeType);
                            fixed (byte* dataPtr = data.Data)
                            {
                                NativeWebView.rnw_request_set_response(
                                    requestId,
                                    data.StatusCode,
                                    (byte*)mimeUtf8,
                                    dataPtr,
                                    data.Data.Length);
                            }
                            Marshal.FreeHGlobal(mimeUtf8);
                            return true;
                        }
                    }
                }

                return false;
            }

            public void AddResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
                lock (_resourceRequestHandlers) _resourceRequestHandlers.Add(handler);
            }

            public void RemoveResourceRequestHandler(Action<IRequestHandlerContext> handler)
            {
                lock (_resourceRequestHandlers) _resourceRequestHandlers.Remove(handler);
            }

            public void AddBeforeBrowseHandler(Action<IBeforeBrowseContext> handler)
            {
                lock (_beforeBrowseHandlers) _beforeBrowseHandlers.Add(handler);
            }

            public void RemoveBeforeBrowseHandler(Action<IBeforeBrowseContext> handler)
            {
                lock (_beforeBrowseHandlers) _beforeBrowseHandlers.Remove(handler);
            }

            public void FocusEntered()
            {
                if (_textInputActive)
                    Owner.Root?.Window?.TextInputStart();
            }

            public void FocusExited()
            {
                if (_textInputActive)
                    Owner.Root?.Window?.TextInputStop();
            }

            public void TextInputStart()
            {
                _textInputActive = true;
                if (Owner.HasKeyboardFocus())
                    Owner.Root?.Window?.TextInputStart();
            }

            public void TextInputStop()
            {
                _textInputActive = false;
                if (Owner.HasKeyboardFocus())
                    Owner.Root?.Window?.TextInputStop();
            }

            // ================================================================
            // Static unmanaged callbacks (called from Rust)
            // ================================================================

            private static unsafe ControlImpl? Resolve(void* userData)
            {
                var handle = GCHandle.FromIntPtr((IntPtr)userData);
                return handle.Target as ControlImpl;
            }

            [UnmanagedCallersOnly]
            private static unsafe void OnPaintCallback(
                void* userData, int width, int height, byte* buffer,
                int dirtyCount, int* dirtyRects)
            {
                var self = Resolve(userData);
                if (self?.Owner.Disposed != false || self._data == null) return;

                for (int i = 0; i < dirtyCount; i++)
                {
                    int x = dirtyRects[i * 4];
                    int y = dirtyRects[i * 4 + 1];
                    int w = dirtyRects[i * 4 + 2];
                    int h = dirtyRects[i * 4 + 3];
                    self._data.Buffer.UpdateBuffer(width, height, (IntPtr)buffer, x, y, w, h);
                }

                self._data.IsDirty = true;
            }

            [UnmanagedCallersOnly]
            private static unsafe void GetViewRectCallback(void* userData, int* outWidth, int* outHeight)
            {
                var self = Resolve(userData);
                if (self?.Owner.Disposed != false)
                {
                    *outWidth = 1;
                    *outHeight = 1;
                    return;
                }

                *outWidth = (int)Math.Max(self.Owner.Size.X, 1);
                *outHeight = (int)Math.Max(self.Owner.Size.Y, 1);
            }

            [UnmanagedCallersOnly]
            private static unsafe void GetScreenInfoCallback(void* userData, float* outScale)
            {
                var self = Resolve(userData);
                if (self?.Owner.Disposed != false)
                {
                    *outScale = 1.0f;
                    return;
                }

                *outScale = self.Owner.UIScale;
            }

            [UnmanagedCallersOnly]
            private static unsafe void OnVirtualKeyboardRequestedCallback(void* userData, int inputMode)
            {
                var self = Resolve(userData);
                if (self == null) return;

                if (inputMode == 0) // None
                    self.TextInputStop();
                else
                    self.TextInputStart();
            }

            [UnmanagedCallersOnly]
            private static unsafe int OnBeforeBrowseCallback(
                void* userData, byte* urlPtr, int userGesture, int isRedirect)
            {
                var self = Resolve(userData);
                if (self == null) return 0;

                var url = Marshal.PtrToStringUTF8((IntPtr)urlPtr) ?? "";
                var context = new NativeBeforeBrowseContext(url, isRedirect != 0, userGesture != 0);

                lock (self._beforeBrowseHandlers)
                {
                    foreach (var handler in self._beforeBrowseHandlers)
                    {
                        handler(context);
                        if (context.IsCancelled)
                            return 1;
                    }
                }

                return 0;
            }

            [UnmanagedCallersOnly]
            private static unsafe int OnResourceRequestCallback(
                void* userData, ulong requestId, byte* urlPtr, byte* methodPtr)
            {
                var self = Resolve(userData);
                if (self == null) return 0;

                var url = Marshal.PtrToStringUTF8((IntPtr)urlPtr) ?? "";
                var method = Marshal.PtrToStringUTF8((IntPtr)methodPtr) ?? "GET";

                // Deny file:// access
                if (url.StartsWith("file://", StringComparison.OrdinalIgnoreCase))
                    return 0;

                var context = new NativeRequestHandlerContext(url, method);

                lock (self._resourceRequestHandlers)
                {
                    foreach (var handler in self._resourceRequestHandlers)
                    {
                        handler(context);

                        if (context.IsCancelled)
                            return 0;

                        if (context.ResponseData != null)
                        {
                            // Set the response via the FFI
                            var data = context.ResponseData;
                            var mimeUtf8 = MarshalStringToUtf8(data.MimeType);
                            fixed (byte* dataPtr = data.Data)
                            {
                                NativeWebView.rnw_request_set_response(
                                    requestId,
                                    data.StatusCode,
                                    (byte*)mimeUtf8,
                                    dataPtr,
                                    data.Data.Length);
                            }
                            Marshal.FreeHGlobal(mimeUtf8);
                            return 1;
                        }
                    }
                }

                return 0;
            }

            [UnmanagedCallersOnly]
            private static unsafe void OnLoadStartCallback(void* userData)
            {
                // Currently unused but required by callback struct.
            }

            [UnmanagedCallersOnly]
            private static unsafe void OnLoadEndCallback(void* userData, int httpStatusCode)
            {
                // Currently unused but required by callback struct.
            }

            // ================================================================
            // Live data
            // ================================================================

            private sealed class LiveData
            {
                public OwnedTexture Texture;
                public readonly ImageBuffer Buffer;
                public volatile bool IsDirty;

                public LiveData(OwnedTexture texture)
                {
                    Texture = texture;
                    Buffer = new ImageBuffer();
                }
            }
        }
    }
}
