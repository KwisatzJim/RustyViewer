// Local libmpv prototype. The main RustyViewer interface remains independent.
#import <Cocoa/Cocoa.h>
#import <OpenGL/gl.h>
#import <mpv/client.h>
#import <mpv/render_gl.h>
#include <dlfcn.h>
#include <mach-o/dyld.h>

static void *resolveGL(void *context, const char *name) {
    (void)context;
    return dlsym(RTLD_DEFAULT, name);
}

static void command(mpv_handle *player, const char **args) {
    if (player) mpv_command_async(player, 0, args);
}

@interface VideoView : NSOpenGLView
@property mpv_render_context *renderer;
@property mpv_handle *player;
@end

@implementation VideoView
- (BOOL)acceptsFirstResponder { return YES; }
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [[self openGLContext] makeCurrentContext];
    if (self.renderer) {
        mpv_render_context_update(self.renderer);
        NSRect pixels = [self convertRectToBacking:self.bounds];
        mpv_opengl_fbo target = {.fbo = 0, .w = (int)pixels.size.width,
                                .h = (int)pixels.size.height};
        int flip = 1;
        mpv_render_param params[] = {
            {MPV_RENDER_PARAM_OPENGL_FBO, &target},
            {MPV_RENDER_PARAM_FLIP_Y, &flip}, {0, NULL}};
        mpv_render_context_render(self.renderer, params);
    } else {
        glClearColor(0, 0, 0, 1);
        glClear(GL_COLOR_BUFFER_BIT);
    }
    [[self openGLContext] flushBuffer];
}
- (void)keyDown:(NSEvent *)event {
    NSString *key = event.charactersIgnoringModifiers.lowercaseString;
    if ([key isEqualToString:@" "]) {
        const char *args[] = {"cycle", "pause", NULL};
        command(self.player, args);
    } else if (event.keyCode == 123 || event.keyCode == 124) {
        const char *args[] = {"seek", event.keyCode == 123 ? "-10" : "10", "relative", NULL};
        command(self.player, args);
    } else if ([key isEqualToString:@"f"]) {
        [self.window toggleFullScreen:nil];
    } else {
        [super keyDown:event];
    }
}
@end

static void redraw(void *context) {
    VideoView *view = (__bridge VideoView *)context;
    dispatch_async(dispatch_get_main_queue(), ^{ [view setNeedsDisplay:YES]; });
}

@interface VideoDelegate : NSObject <NSApplicationDelegate, NSWindowDelegate>
@property NSWindow *window;
@property VideoView *view;
@property NSButton *pauseButton;
@property NSButton *subtitleButton;
@property NSTextField *status;
@property NSTimer *timer;
@property mpv_handle *player;
@property BOOL stopping;
@end

@implementation VideoDelegate
- (void)fail:(NSString *)message {
    fprintf(stderr, "Native video error: %s\n", message.UTF8String);
    NSAlert *alert = [NSAlert new];
    alert.messageText = @"Video playback failed";
    alert.informativeText = message;
    [alert runModal];
    [NSApp terminate:nil];
}
- (NSButton *)button:(NSString *)title x:(CGFloat)x action:(SEL)action {
    NSButton *button = [NSButton buttonWithTitle:title target:self action:action];
    button.frame = NSMakeRect(x, 8, 92, 30);
    [self.window.contentView addSubview:button];
    return button;
}
- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    (void)notification;
    NSArray<NSString *> *args = NSProcessInfo.processInfo.arguments;
    NSString *path;
    if (args.count == 2) {
        path = args[1].stringByStandardizingPath;
    } else if (args.count == 1) {
        NSOpenPanel *panel = [NSOpenPanel openPanel];
        panel.title = @"Choose a video for the libmpv prototype";
        panel.prompt = @"Play video";
        panel.canChooseDirectories = NO;
        panel.allowsMultipleSelection = NO;
        if ([panel runModal] != NSModalResponseOK) { [NSApp terminate:nil]; return; }
        path = panel.URL.path;
    } else { [self fail:@"Provide one local video path."]; return; }
    BOOL directory = NO;
    if (![NSFileManager.defaultManager fileExistsAtPath:path isDirectory:&directory] || directory) {
        [self fail:@"The selected video file does not exist."]; return;
    }
    self.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 960, 600)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                  NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered defer:NO];
    self.window.title = [@"RustyViewer Video — " stringByAppendingString:path.lastPathComponent];
    self.window.minSize = NSMakeSize(820, 300);
    self.window.delegate = self;
    self.window.releasedWhenClosed = NO;
    NSOpenGLPixelFormatAttribute attributes[] = {
        NSOpenGLPFADoubleBuffer, NSOpenGLPFAAccelerated, 0};
    NSOpenGLPixelFormat *format = [[NSOpenGLPixelFormat alloc] initWithAttributes:attributes];
    self.view = [[VideoView alloc] initWithFrame:NSMakeRect(0, 48, 960, 552) pixelFormat:format];
    self.view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    self.view.wantsBestResolutionOpenGLSurface = YES;
    [self.window.contentView addSubview:self.view];
    [[self.view openGLContext] makeCurrentContext];
    GLint interval = 1;
    [[self.view openGLContext] setValues:&interval forParameter:NSOpenGLCPSwapInterval];
    self.pauseButton = [self button:@"Pause" x:8 action:@selector(togglePause:)];
    [self button:@"−10 sec" x:106 action:@selector(back:)];
    [self button:@"+10 sec" x:204 action:@selector(forward:)];
    [self button:@"Fullscreen" x:302 action:@selector(fullscreen:)];
    NSSlider *volume = [NSSlider sliderWithValue:100 minValue:0 maxValue:100 target:self action:@selector(volume:)];
    volume.frame = NSMakeRect(406, 13, 100, 22);
    volume.toolTip = @"Volume";
    [self.window.contentView addSubview:volume];
    self.subtitleButton = [NSButton checkboxWithTitle:@"Subtitles" target:self action:@selector(subtitles:)];
    self.subtitleButton.frame = NSMakeRect(518, 12, 100, 24);
    self.subtitleButton.state = NSControlStateValueOn;
    self.subtitleButton.toolTip = @"Show or hide the selected subtitle track";
    [self.window.contentView addSubview:self.subtitleButton];
    self.status = [NSTextField labelWithString:@"Loading…"];
    self.status.frame = NSMakeRect(628, 15, 320, 20);
    self.status.autoresizingMask = NSViewWidthSizable;
    [self.window.contentView addSubview:self.status];
    self.player = mpv_create();
    if (!self.player) { [self fail:@"Could not create libmpv."]; return; }
    for (NSArray<NSString *> *option in @[@[@"config", @"no"], @[@"vo", @"libmpv"],
         @[@"hwdec", @"auto-safe"], @[@"keep-open", @"yes"]]) {
        int result = mpv_set_option_string(self.player, option[0].UTF8String, option[1].UTF8String);
        if (result < 0) { [self fail:@(mpv_error_string(result))]; return; }
    }
    int result = mpv_initialize(self.player);
    if (result < 0) { [self fail:@(mpv_error_string(result))]; return; }
    mpv_opengl_init_params gl = {.get_proc_address = resolveGL};
    mpv_render_param params[] = {
        {MPV_RENDER_PARAM_API_TYPE, (void *)MPV_RENDER_API_TYPE_OPENGL},
        {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl}, {0, NULL}};
    mpv_render_context *renderer = NULL;
    result = mpv_render_context_create(&renderer, self.player, params);
    if (result < 0) { [self fail:@(mpv_error_string(result))]; return; }
    self.view.renderer = renderer;
    self.view.player = self.player;
    mpv_render_context_set_update_callback(renderer, redraw, (__bridge void *)self.view);
    self.timer = [NSTimer scheduledTimerWithTimeInterval:0.1 target:self selector:@selector(poll:) userInfo:nil repeats:YES];
    [self.window center];
    [self.window makeKeyAndOrderFront:nil];
    [self.window makeFirstResponder:self.view];
    [NSApp activateIgnoringOtherApps:YES];
    const char *load[] = {"loadfile", path.UTF8String, NULL};
    command(self.player, load);
}
- (void)poll:(NSTimer *)timer {
    (void)timer;
    if (!self.player || self.stopping) return;
    for (;;) {
        mpv_event *event = mpv_wait_event(self.player, 0);
        if (event->event_id == MPV_EVENT_NONE) break;
        if (event->event_id == MPV_EVENT_FILE_LOADED) fprintf(stderr, "Native video: file loaded\n");
        if (event->event_id == MPV_EVENT_END_FILE) {
            mpv_event_end_file *end = event->data;
            if (end->reason == MPV_END_FILE_REASON_ERROR) {
                [self fail:@(mpv_error_string(end->error))]; return;
            }
        }
        if (event->event_id == MPV_EVENT_COMMAND_REPLY && event->error < 0) {
            fprintf(stderr, "Native video command: %s\n", mpv_error_string(event->error));
        }
    }
    double position = 0, duration = 0;
    int paused = 0, subtitlesVisible = 1;
    mpv_get_property(self.player, "time-pos", MPV_FORMAT_DOUBLE, &position);
    mpv_get_property(self.player, "duration", MPV_FORMAT_DOUBLE, &duration);
    mpv_get_property(self.player, "pause", MPV_FORMAT_FLAG, &paused);
    self.pauseButton.title = paused ? @"Play" : @"Pause";
    if (mpv_get_property(self.player, "sub-visibility", MPV_FORMAT_FLAG, &subtitlesVisible) >= 0)
        self.subtitleButton.state = subtitlesVisible ? NSControlStateValueOn : NSControlStateValueOff;
    self.status.stringValue = [NSString stringWithFormat:@"%.0f / %.0f sec · Space pauses · F fullscreen", position, duration];
}
- (void)togglePause:(id)sender {
    (void)sender;
    const char *args[] = {"cycle", "pause", NULL}; command(self.player, args);
}
- (void)back:(id)sender {
    (void)sender;
    const char *args[] = {"seek", "-10", "relative", NULL}; command(self.player, args);
}
- (void)forward:(id)sender {
    (void)sender;
    const char *args[] = {"seek", "10", "relative", NULL}; command(self.player, args);
}
- (void)fullscreen:(id)sender { [self.window toggleFullScreen:sender]; }
- (void)subtitles:(NSButton *)sender {
    const char *args[] = {"set", "sub-visibility",
        sender.state == NSControlStateValueOn ? "yes" : "no", NULL};
    command(self.player, args);
}
- (void)volume:(NSSlider *)sender {
    NSString *value = [NSString stringWithFormat:@"%.0f", sender.doubleValue];
    const char *args[] = {"set", "volume", value.UTF8String, NULL}; command(self.player, args);
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    (void)sender; return YES;
}
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    (void)sender;
    self.stopping = YES;
    [self.timer invalidate];
    if (self.view.renderer) {
        [[self.view openGLContext] makeCurrentContext];
        mpv_render_context_set_update_callback(self.view.renderer, NULL, NULL);
        mpv_render_context_free(self.view.renderer);
        self.view.renderer = NULL;
    }
    self.view.player = NULL;
    if (self.player) { mpv_terminate_destroy(self.player); self.player = NULL; }
    return NSTerminateNow;
}
@end

int main(int argc, const char *argv[]) {
    if (argc == 2 && strcmp(argv[1], "--check-bundle") == 0) {
        mpv_handle *player = mpv_create();
        if (!player) return 1;
        mpv_set_option_string(player, "config", "no");
        mpv_set_option_string(player, "vo", "null");
        mpv_set_option_string(player, "ao", "null");
        int result = mpv_initialize(player);
        if (result < 0) {
            fprintf(stderr, "libmpv initialization failed: %s\n", mpv_error_string(result));
            mpv_terminate_destroy(player);
            return 1;
        }
        for (uint32_t i = 0; i < _dyld_image_count(); i++)
            printf("LIBRARY\t%s\n", _dyld_get_image_name(i));
        mpv_terminate_destroy(player);
        return 0;
    }
    @autoreleasepool {
        NSApplication *app = NSApplication.sharedApplication;
        [app setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSMenu *menu = [NSMenu new];
        NSMenuItem *item = [NSMenuItem new];
        NSMenu *submenu = [NSMenu new];
        [submenu addItemWithTitle:@"Quit Video Player" action:@selector(terminate:) keyEquivalent:@"q"];
        item.submenu = submenu; [menu addItem:item]; app.mainMenu = menu;
        VideoDelegate *delegate = [VideoDelegate new];
        app.delegate = delegate;
        [app run];
    }
    return 0;
}
