// macOS WKWebView implementation
// Requires: WebKit.framework (ships with macOS)

#if defined(__APPLE__) && defined(__MACH__)

#import <WebKit/WebKit.h>
#import <Cocoa/Cocoa.h>

// Callback types matching Rust signatures
typedef void (*SchemeCallback)(const char* url, void* request_handle, void* user_data);
typedef void (*MessageCallback)(void* handle, const char* message, void* user_data);

// Global state
static SchemeCallback g_scheme_callback = NULL;
static void* g_scheme_user_data = NULL;
static BOOL g_initialized = NO;

// Forward declarations
@class RobustSchemeHandler;
@class RobustScriptMessageHandler;

// Per-webview state
typedef struct {
    WKWebView* webview;
    NSWindow* parent;
    RobustScriptMessageHandler* message_handler;
    MessageCallback message_callback;
    void* message_user_data;
} WebViewInstance;

// Custom URL scheme handler for res://
@interface RobustSchemeHandler : NSObject <WKURLSchemeHandler>
@end

@implementation RobustSchemeHandler

- (void)webView:(WKWebView *)webView startURLSchemeTask:(id<WKURLSchemeTask>)urlSchemeTask {
    if (!g_scheme_callback) {
        NSError* error = [NSError errorWithDomain:NSURLErrorDomain code:NSURLErrorFileDoesNotExist userInfo:nil];
        [urlSchemeTask didFailWithError:error];
        return;
    }

    NSURL* url = urlSchemeTask.request.URL;
    const char* urlString = [[url absoluteString] UTF8String];

    // Prevent ARC from releasing the task while we wait for response
    CFRetain((__bridge CFTypeRef)urlSchemeTask);

    g_scheme_callback(urlString, (__bridge void*)urlSchemeTask, g_scheme_user_data);
}

- (void)webView:(WKWebView *)webView stopURLSchemeTask:(id<WKURLSchemeTask>)urlSchemeTask {
    // Request cancelled - nothing to do
}

@end

// Script message handler for JS -> native communication
@interface RobustScriptMessageHandler : NSObject <WKScriptMessageHandler>
@property (nonatomic, assign) WebViewInstance* instance;
@end

@implementation RobustScriptMessageHandler

- (void)userContentController:(WKUserContentController *)userContentController
      didReceiveScriptMessage:(WKScriptMessage *)message {
    if (!self.instance || !self.instance->message_callback) return;

    NSString* body = nil;
    if ([message.body isKindOfClass:[NSString class]]) {
        body = message.body;
    } else {
        NSData* jsonData = [NSJSONSerialization dataWithJSONObject:message.body options:0 error:nil];
        if (jsonData) {
            body = [[NSString alloc] initWithData:jsonData encoding:NSUTF8StringEncoding];
        }
    }

    if (body) {
        self.instance->message_callback(self.instance, [body UTF8String], self.instance->message_user_data);
    }
}

@end

// Static scheme handler instance
static RobustSchemeHandler* g_scheme_handler = nil;

#pragma mark - C API

int webview_mac_init(void) {
    if (g_initialized) return 0;

    g_scheme_handler = [[RobustSchemeHandler alloc] init];
    g_initialized = YES;
    return 0;
}

void webview_mac_shutdown(void) {
    g_scheme_handler = nil;
    g_scheme_callback = NULL;
    g_scheme_user_data = NULL;
    g_initialized = NO;
}

void webview_mac_pump(void) {
    // WKWebView uses the NSRunLoop, which SDL already runs
    // No additional pumping needed
}

void* webview_mac_create(void* parent_handle, const char* url) {
    if (!g_initialized || !parent_handle) return NULL;

    @autoreleasepool {
        NSWindow* parent = (__bridge NSWindow*)parent_handle;

        WebViewInstance* instance = (WebViewInstance*)calloc(1, sizeof(WebViewInstance));
        instance->parent = parent;

        // Configure webview
        WKWebViewConfiguration* config = [[WKWebViewConfiguration alloc] init];

        // Register res:// scheme handler
        [config setURLSchemeHandler:g_scheme_handler forURLScheme:@"res"];

        // Set up message handler
        instance->message_handler = [[RobustScriptMessageHandler alloc] init];
        instance->message_handler.instance = instance;
        [config.userContentController addScriptMessageHandler:instance->message_handler name:@"robust"];

        // Create webview with zero frame - caller sets bounds via set_bounds
        NSView* contentView = [parent contentView];
        NSRect frame = NSMakeRect(0, 0, 0, 0);

        instance->webview = [[WKWebView alloc] initWithFrame:frame configuration:config];

        // Add as subview
        [contentView addSubview:instance->webview];

        // Navigate to initial URL
        if (url && url[0]) {
            NSString* urlString = [NSString stringWithUTF8String:url];
            NSURL* nsurl = [NSURL URLWithString:urlString];
            if (nsurl) {
                [instance->webview loadRequest:[NSURLRequest requestWithURL:nsurl]];
            }
        }

        return instance;
    }
}

void webview_mac_destroy(void* handle) {
    if (!handle) return;

    @autoreleasepool {
        WebViewInstance* instance = (WebViewInstance*)handle;

        if (instance->webview) {
            [instance->webview removeFromSuperview];
            instance->webview = nil;
        }

        instance->message_handler = nil;
        free(instance);
    }
}

void webview_mac_navigate(void* handle, const char* url) {
    if (!handle || !url) return;

    @autoreleasepool {
        WebViewInstance* instance = (WebViewInstance*)handle;
        NSString* urlString = [NSString stringWithUTF8String:url];
        NSURL* nsurl = [NSURL URLWithString:urlString];
        if (nsurl && instance->webview) {
            [instance->webview loadRequest:[NSURLRequest requestWithURL:nsurl]];
        }
    }
}

void webview_mac_reload(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    [instance->webview reload];
}

void webview_mac_stop(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    [instance->webview stopLoading];
}

void webview_mac_go_back(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    [instance->webview goBack];
}

void webview_mac_go_forward(void* handle) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    [instance->webview goForward];
}

int webview_mac_can_go_back(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return instance->webview.canGoBack ? 1 : 0;
}

int webview_mac_can_go_forward(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return instance->webview.canGoForward ? 1 : 0;
}

int webview_mac_is_loading(void* handle) {
    if (!handle) return 0;
    WebViewInstance* instance = (WebViewInstance*)handle;
    return instance->webview.isLoading ? 1 : 0;
}

void webview_mac_execute_js(void* handle, const char* code) {
    if (!handle || !code) return;

    @autoreleasepool {
        WebViewInstance* instance = (WebViewInstance*)handle;
        NSString* jsCode = [NSString stringWithUTF8String:code];
        [instance->webview evaluateJavaScript:jsCode completionHandler:nil];
    }
}

void webview_mac_set_size(void* handle, int width, int height) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    NSRect oldFrame = instance->webview.frame;
    NSRect frame = NSMakeRect(oldFrame.origin.x, oldFrame.origin.y, width, height);
    [instance->webview setFrame:frame];
}

void webview_mac_set_bounds(void* handle, int x, int y, int width, int height) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    NSView* contentView = [instance->parent contentView];
    // macOS NSView origin is bottom-left, but game UI uses top-left origin
    CGFloat parentHeight = contentView.bounds.size.height;
    NSRect frame = NSMakeRect(x, parentHeight - y - height, width, height);
    [instance->webview setFrame:frame];
}

void webview_mac_set_scheme_handler(
    void (*callback)(const char*, void*, void*),
    void* user_data
) {
    g_scheme_callback = callback;
    g_scheme_user_data = user_data;
}

void webview_mac_respond_scheme(
    void* request_handle,
    const void* data,
    int length,
    const char* mime_type,
    int status_code
) {
    if (!request_handle) return;

    @autoreleasepool {
        id<WKURLSchemeTask> task = (__bridge_transfer id<WKURLSchemeTask>)request_handle;

        NSData* responseData = [NSData dataWithBytes:data length:length];
        NSString* mimeString = mime_type ? [NSString stringWithUTF8String:mime_type] : @"application/octet-stream";

        NSHTTPURLResponse* response = [[NSHTTPURLResponse alloc]
            initWithURL:task.request.URL
            statusCode:status_code
            HTTPVersion:@"HTTP/1.1"
            headerFields:@{@"Content-Type": mimeString, @"Content-Length": [@(length) stringValue]}];

        @try {
            [task didReceiveResponse:response];
            [task didReceiveData:responseData];
            [task didFinish];
        } @catch (NSException* e) {
            // Task may have been cancelled
        }
    }
}

void webview_mac_set_message_handler(
    void* handle,
    void (*callback)(void*, const char*, void*),
    void* user_data
) {
    if (!handle) return;
    WebViewInstance* instance = (WebViewInstance*)handle;
    instance->message_callback = callback;
    instance->message_user_data = user_data;
}

#endif // __APPLE__
