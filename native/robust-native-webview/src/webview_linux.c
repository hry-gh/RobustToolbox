// Linux WebKitGTK implementation
// Requires: libwebkit2gtk-4.1 or libwebkit2gtk-4.0

#if defined(__linux__)

#include <gtk/gtk.h>
#include <webkit2/webkit2.h>
#include <stdlib.h>
#include <string.h>

// Callback types matching Rust signatures
typedef void (*SchemeCallback)(const char* url, void* request_handle, void* user_data);
typedef void (*MessageCallback)(void* handle, const char* message, void* user_data);

// Global state
static SchemeCallback g_scheme_callback = NULL;
static void* g_scheme_user_data = NULL;
static WebKitWebContext* g_web_context = NULL;
static int g_initialized = 0;

// Per-webview state
typedef struct {
    GtkWidget* plug;
    WebKitWebView* webview;
    WebKitUserContentManager* content_manager;
    MessageCallback message_callback;
    void* message_user_data;
} WebViewInstance;

// Scheme handler callback
static void scheme_request_callback(WebKitURISchemeRequest* request, gpointer user_data) {
    if (!g_scheme_callback) {
        GError* error = g_error_new(WEBKIT_NETWORK_ERROR, WEBKIT_NETWORK_ERROR_FILE_DOES_NOT_EXIST, "No handler");
        webkit_uri_scheme_request_finish_error(request, error);
        g_error_free(error);
        return;
    }

    const char* uri = webkit_uri_scheme_request_get_uri(request);

    // Prevent GC while waiting for response
    g_object_ref(request);

    g_scheme_callback(uri, request, g_scheme_user_data);
}

// Script message callback
static void script_message_callback(WebKitUserContentManager* manager,
                                     WebKitJavascriptResult* result,
                                     gpointer user_data) {
    WebViewInstance* instance = (WebViewInstance*)user_data;
    if (!instance || !instance->message_callback) return;

    JSCValue* value = webkit_javascript_result_get_js_value(result);
    if (jsc_value_is_string(value)) {
        char* message = jsc_value_to_string(value);
        instance->message_callback(instance, message, instance->message_user_data);
        g_free(message);
    }
}

#pragma GCC visibility push(default)

int webview_linux_init(void) {
    if (g_initialized) return 0;

    // Initialize GTK (may already be initialized)
    if (!gtk_init_check(NULL, NULL)) {
        return -1;
    }

    // Create web context with scheme handler
    g_web_context = webkit_web_context_new();
    webkit_web_context_register_uri_scheme(g_web_context, "res", scheme_request_callback, NULL, NULL);

    g_initialized = 1;
    return 0;
}

void webview_linux_shutdown(void) {
    if (g_web_context) {
        g_object_unref(g_web_context);
        g_web_context = NULL;
    }
    g_scheme_callback = NULL;
    g_scheme_user_data = NULL;
    g_initialized = 0;
}

void webview_linux_pump(void) {
    // Process pending GTK events without blocking
    while (gtk_events_pending()) {
        gtk_main_iteration_do(FALSE);
    }
}

void* webview_linux_create(void* parent_handle, const char* url) {
    if (!g_initialized || !parent_handle) return NULL;

    // parent_handle is an X11 window ID
    Window x11_window = (Window)(uintptr_t)parent_handle;

    WebViewInstance* instance = calloc(1, sizeof(WebViewInstance));
    if (!instance) return NULL;

    // Create GtkPlug to embed in X11 window
    instance->plug = gtk_plug_new(x11_window);

    // Create content manager for script messages
    instance->content_manager = webkit_user_content_manager_new();
    webkit_user_content_manager_register_script_message_handler(instance->content_manager, "robust");
    g_signal_connect(instance->content_manager, "script-message-received::robust",
                     G_CALLBACK(script_message_callback), instance);

    // Create webview
    instance->webview = WEBKIT_WEB_VIEW(webkit_web_view_new_with_context(g_web_context));
    webkit_web_view_set_user_content_manager(instance->webview, instance->content_manager);

    // Add webview to plug
    gtk_container_add(GTK_CONTAINER(instance->plug), GTK_WIDGET(instance->webview));

    // Show widgets
    gtk_widget_show_all(instance->plug);

    // Navigate to initial URL
    if (url && url[0]) {
        webkit_web_view_load_uri(instance->webview, url);
    }

    return instance;
}

void webview_linux_destroy(void* handle) {
    if (!handle) return;

    WebViewInstance* instance = (WebViewInstance*)handle;

    if (instance->plug) {
        gtk_widget_destroy(instance->plug);
    }

    if (instance->content_manager) {
        g_object_unref(instance->content_manager);
    }

    free(instance);
}

void webview_linux_navigate(void* handle, const char* url) {
    if (!handle || !url) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_load_uri(instance->webview, url);
}

void webview_linux_reload(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_reload(instance->webview);
}

void webview_linux_stop(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_stop_loading(instance->webview);
}

void webview_linux_go_back(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_go_back(instance->webview);
}

void webview_linux_go_forward(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_go_forward(instance->webview);
}

int webview_linux_can_go_back(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return webkit_web_view_can_go_back(instance->webview) ? 1 : 0;
}

int webview_linux_can_go_forward(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return webkit_web_view_can_go_forward(instance->webview) ? 1 : 0;
}

int webview_linux_is_loading(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return webkit_web_view_is_loading(instance->webview) ? 1 : 0;
}

void webview_linux_execute_js(void* handle, const char* code) {
    if (!handle || !code) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    webkit_web_view_run_javascript(instance->webview, code, NULL, NULL, NULL);
}

void webview_linux_set_size(void* handle, int width, int height) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    gtk_widget_set_size_request(GTK_WIDGET(instance->webview), width, height);
}

void webview_linux_set_scheme_handler(
    void (*callback)(const char*, void*, void*),
    void* user_data
) {
    g_scheme_callback = callback;
    g_scheme_user_data = user_data;
}

void webview_linux_respond_scheme(
    void* request_handle,
    const void* data,
    int length,
    const char* mime_type,
    int status_code
) {
    if (!request_handle) return;

    WebKitURISchemeRequest* request = (WebKitURISchemeRequest*)request_handle;

    GInputStream* stream = g_memory_input_stream_new_from_data(
        g_memdup2(data, length),
        length,
        g_free
    );

    webkit_uri_scheme_request_finish(request, stream, length, mime_type ? mime_type : "application/octet-stream");

    g_object_unref(stream);
    g_object_unref(request);
}

void webview_linux_set_message_handler(
    void* handle,
    void (*callback)(void*, const char*, void*),
    void* user_data
) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    instance->message_callback = callback;
    instance->message_user_data = user_data;
}

#pragma GCC visibility pop

#endif // __linux__
