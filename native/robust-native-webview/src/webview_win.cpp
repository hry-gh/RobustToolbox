// Windows WebView2 implementation
// Requires: WebView2Loader.lib (static), Windows SDK

#ifdef _WIN32

#include <windows.h>
#include <wrl.h>
#include <WebView2.h>
#include <string>
#include <unordered_map>

using namespace Microsoft::WRL;

// Callback types matching Rust signatures
typedef void (*SchemeCallback)(const char* url, void* request_handle, void* user_data);
typedef void (*MessageCallback)(void* handle, const char* message, void* user_data);

// Global state
static ComPtr<ICoreWebView2Environment> g_environment;
static SchemeCallback g_scheme_callback = nullptr;
static void* g_scheme_user_data = nullptr;
static bool g_initialized = false;

// Per-webview state
struct WebViewInstance {
    ComPtr<ICoreWebView2Controller> controller;
    ComPtr<ICoreWebView2> webview;
    HWND parent;
    MessageCallback message_callback;
    void* message_user_data;
};

static std::unordered_map<void*, WebViewInstance*> g_instances;

// Helper to convert UTF-8 to wide string
static std::wstring Utf8ToWide(const char* utf8) {
    if (!utf8 || !*utf8) return L"";
    int len = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, nullptr, 0);
    std::wstring result(len - 1, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, &result[0], len);
    return result;
}

// Helper to convert wide string to UTF-8
static std::string WideToUtf8(const wchar_t* wide) {
    if (!wide || !*wide) return "";
    int len = WideCharToMultiByte(CP_UTF8, 0, wide, -1, nullptr, 0, nullptr, nullptr);
    std::string result(len - 1, '\0');
    WideCharToMultiByte(CP_UTF8, 0, wide, -1, &result[0], len, nullptr, nullptr);
    return result;
}

extern "C" {

int webview_win_init() {
    if (g_initialized) return 0;

    HANDLE hEvent = CreateEvent(nullptr, TRUE, FALSE, nullptr);
    if (!hEvent) return -1;

    HRESULT hr = CreateCoreWebView2EnvironmentWithOptions(
        nullptr, nullptr, nullptr,
        Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
            [hEvent](HRESULT result, ICoreWebView2Environment* env) -> HRESULT {
                if (SUCCEEDED(result)) {
                    g_environment = env;
                }
                SetEvent(hEvent);
                return S_OK;
            }
        ).Get()
    );

    if (FAILED(hr)) {
        CloseHandle(hEvent);
        return -1;
    }

    // Wait for environment with message pumping
    while (!g_environment) {
        DWORD result = MsgWaitForMultipleObjects(1, &hEvent, FALSE, 5000, QS_ALLINPUT);
        if (result == WAIT_OBJECT_0 || result == WAIT_TIMEOUT) break;
        MSG msg;
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
    }

    CloseHandle(hEvent);
    g_initialized = g_environment != nullptr;
    return g_initialized ? 0 : -1;
}

void webview_win_shutdown() {
    for (auto& pair : g_instances) {
        delete pair.second;
    }
    g_instances.clear();
    g_environment.Reset();
    g_initialized = false;
}

void webview_win_pump() {
    // WebView2 uses the Win32 message loop, which SDL already runs
    // No additional pumping needed
}

void* webview_win_create(void* parent_handle, const char* url) {
    if (!g_initialized || !parent_handle) return nullptr;

    HWND parent = (HWND)parent_handle;
    WebViewInstance* instance = new WebViewInstance();
    instance->parent = parent;
    instance->message_callback = nullptr;
    instance->message_user_data = nullptr;

    HANDLE hEvent = CreateEvent(nullptr, TRUE, FALSE, nullptr);
    if (!hEvent) {
        delete instance;
        return nullptr;
    }

    HRESULT hr = g_environment->CreateCoreWebView2Controller(
        parent,
        Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
            [instance, url, hEvent](HRESULT result, ICoreWebView2Controller* controller) -> HRESULT {
                if (FAILED(result) || !controller) {
                    SetEvent(hEvent);
                    return S_OK;
                }

                instance->controller = controller;
                controller->get_CoreWebView2(&instance->webview);

                // Fill parent window
                RECT bounds;
                GetClientRect(instance->parent, &bounds);
                controller->put_Bounds(bounds);

                // Register res:// scheme handler
                if (g_scheme_callback) {
                    instance->webview->AddWebResourceRequestedFilter(L"res://*", COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
                    instance->webview->add_WebResourceRequested(
                        Callback<ICoreWebView2WebResourceRequestedEventHandler>(
                            [](ICoreWebView2* sender, ICoreWebView2WebResourceRequestedEventArgs* args) -> HRESULT {
                                ComPtr<ICoreWebView2WebResourceRequest> request;
                                args->get_Request(&request);

                                LPWSTR uri;
                                request->get_Uri(&uri);
                                std::string uriUtf8 = WideToUtf8(uri);
                                CoTaskMemFree(uri);

                                // Store args for response
                                args->AddRef();
                                g_scheme_callback(uriUtf8.c_str(), args, g_scheme_user_data);
                                return S_OK;
                            }
                        ).Get(),
                        nullptr
                    );
                }

                // Navigate to initial URL
                if (url && *url) {
                    instance->webview->Navigate(Utf8ToWide(url).c_str());
                }

                SetEvent(hEvent);
                return S_OK;
            }
        ).Get()
    );

    if (FAILED(hr)) {
        CloseHandle(hEvent);
        delete instance;
        return nullptr;
    }

    // Wait for controller with message pumping
    while (!instance->controller) {
        DWORD result = MsgWaitForMultipleObjects(1, &hEvent, FALSE, 5000, QS_ALLINPUT);
        if (result == WAIT_OBJECT_0 || result == WAIT_TIMEOUT) break;
        MSG msg;
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
    }

    CloseHandle(hEvent);

    if (!instance->controller) {
        delete instance;
        return nullptr;
    }

    void* handle = (void*)instance;
    g_instances[handle] = instance;
    return handle;
}

void webview_win_destroy(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end()) return;

    WebViewInstance* instance = it->second;
    if (instance->controller) {
        instance->controller->Close();
    }
    delete instance;
    g_instances.erase(it);
}

void webview_win_navigate(void* handle, const char* url) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->Navigate(Utf8ToWide(url).c_str());
}

void webview_win_reload(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->Reload();
}

void webview_win_stop(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->Stop();
}

void webview_win_go_back(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->GoBack();
}

void webview_win_go_forward(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->GoForward();
}

int webview_win_can_go_back(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return 0;
    BOOL result = FALSE;
    it->second->webview->get_CanGoBack(&result);
    return result ? 1 : 0;
}

int webview_win_can_go_forward(void* handle) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return 0;
    BOOL result = FALSE;
    it->second->webview->get_CanGoForward(&result);
    return result ? 1 : 0;
}

int webview_win_is_loading(void* handle) {
    // WebView2 doesn't have a direct IsLoading property
    // Would need to track via navigation events
    return 0;
}

void webview_win_execute_js(void* handle, const char* code) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->webview) return;
    it->second->webview->ExecuteScript(Utf8ToWide(code).c_str(), nullptr);
}

void webview_win_set_size(void* handle, int width, int height) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->controller) return;
    RECT bounds = { 0, 0, width, height };
    it->second->controller->put_Bounds(bounds);
}

void webview_win_set_bounds(void* handle, int x, int y, int width, int height) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end() || !it->second->controller) return;
    RECT bounds = { x, y, x + width, y + height };
    it->second->controller->put_Bounds(bounds);
}

void webview_win_set_scheme_handler(
    void (*callback)(const char*, void*, void*),
    void* user_data
) {
    g_scheme_callback = callback;
    g_scheme_user_data = user_data;
}

void webview_win_respond_scheme(
    void* request_handle,
    const void* data,
    int length,
    const char* mime_type,
    int status_code
) {
    ICoreWebView2WebResourceRequestedEventArgs* args =
        (ICoreWebView2WebResourceRequestedEventArgs*)request_handle;

    if (!args) return;

    ComPtr<IStream> stream;
    stream.Attach(SHCreateMemStream((const BYTE*)data, length));

    std::wstring mimeWide = Utf8ToWide(mime_type);
    std::wstring headers = L"Content-Type: " + mimeWide;

    ComPtr<ICoreWebView2WebResourceResponse> response;
    g_environment->CreateWebResourceResponse(
        stream.Get(),
        status_code,
        L"OK",
        headers.c_str(),
        &response
    );

    args->put_Response(response.Get());
    args->Release();
}

void webview_win_set_message_handler(
    void* handle,
    void (*callback)(void*, const char*, void*),
    void* user_data
) {
    auto it = g_instances.find(handle);
    if (it == g_instances.end()) return;

    WebViewInstance* instance = it->second;
    instance->message_callback = callback;
    instance->message_user_data = user_data;

    if (instance->webview && callback) {
        instance->webview->add_WebMessageReceived(
            Callback<ICoreWebView2WebMessageReceivedEventHandler>(
                [instance](ICoreWebView2* sender, ICoreWebView2WebMessageReceivedEventArgs* args) -> HRESULT {
                    if (!instance->message_callback) return S_OK;

                    LPWSTR message;
                    args->TryGetWebMessageAsString(&message);
                    std::string messageUtf8 = WideToUtf8(message);
                    CoTaskMemFree(message);

                    instance->message_callback(instance, messageUtf8.c_str(), instance->message_user_data);
                    return S_OK;
                }
            ).Get(),
            nullptr
        );
    }
}

} // extern "C"

#endif // _WIN32
