#include <gtk/gtk.h>
#include <epoxy/gl.h>
#include <mpv/client.h>
#include <mpv/render_gl.h>
#include <dlfcn.h>
#include <locale.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    GtkWidget *window, *video, *pause, *subtitles, *status;
    mpv_handle *player;
    mpv_render_context *renderer;
    char *path;
    guint timer;
    gboolean fullscreen, syncing;
} Player;

static void command(Player *p, const char **args) {
    if (p->player) mpv_command_async(p->player, 0, args);
}

static void error_dialog(Player *p, const char *message) {
    fprintf(stderr, "Native video: %s\n", message);
    GtkWidget *dialog = gtk_message_dialog_new(GTK_WINDOW(p->window), GTK_DIALOG_MODAL,
        GTK_MESSAGE_ERROR, GTK_BUTTONS_CLOSE, "%s", message);
    g_signal_connect_swapped(dialog, "response", G_CALLBACK(gtk_widget_destroy), dialog);
    gtk_widget_show(dialog);
}

static void *resolve_gl(void *context, const char *name) {
    (void)context;
    void *address = dlsym(RTLD_DEFAULT, name);
    if (address) return address;
    typedef void *(*Resolver)(const char *);
    Resolver egl = (Resolver)dlsym(RTLD_DEFAULT, "eglGetProcAddress");
    if (egl && (address = egl(name))) return address;
    Resolver glx = (Resolver)dlsym(RTLD_DEFAULT, "glXGetProcAddressARB");
    return glx ? glx(name) : NULL;
}

static void realize(GtkGLArea *area, Player *p) {
    gtk_gl_area_make_current(area);
    if (gtk_gl_area_get_error(area)) {
        error_dialog(p, gtk_gl_area_get_error(area)->message);
        return;
    }
    mpv_opengl_init_params gl = {.get_proc_address = resolve_gl};
    mpv_render_param params[] = {
        {MPV_RENDER_PARAM_API_TYPE, MPV_RENDER_API_TYPE_OPENGL},
        {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl}, {0, NULL}};
    int result = mpv_render_context_create(&p->renderer, p->player, params);
    if (result < 0) {
        error_dialog(p, mpv_error_string(result));
        return;
    }
    const char *args[] = {"loadfile", p->path, NULL};
    command(p, args);
}

static gboolean render(GtkGLArea *area, GdkGLContext *context, Player *p) {
    (void)context;
    if (!p->renderer) return TRUE;
    GLint framebuffer;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &framebuffer);
    int scale = gtk_widget_get_scale_factor(GTK_WIDGET(area));
    mpv_opengl_fbo fbo = {.fbo = framebuffer,
        .w = gtk_widget_get_allocated_width(GTK_WIDGET(area)) * scale,
        .h = gtk_widget_get_allocated_height(GTK_WIDGET(area)) * scale};
    int flip = 1;
    mpv_render_param params[] = {{MPV_RENDER_PARAM_OPENGL_FBO, &fbo},
        {MPV_RENDER_PARAM_FLIP_Y, &flip}, {0, NULL}};
    mpv_render_context_update(p->renderer);
    mpv_render_context_render(p->renderer, params);
    return TRUE;
}

static void unrealize(GtkGLArea *area, Player *p) {
    gtk_gl_area_make_current(area);
    if (p->renderer) {
        mpv_render_context_free(p->renderer);
        p->renderer = NULL;
    }
}

static void pause_video(GtkButton *button, Player *p) {
    (void)button;
    const char *args[] = {"cycle", "pause", NULL}; command(p, args);
}

static void seek(GtkButton *button, Player *p) {
    const char *args[] = {"seek", g_object_get_data(G_OBJECT(button), "seconds"), "relative", NULL};
    command(p, args);
}

static void fullscreen(GtkButton *button, Player *p) {
    (void)button;
    if (p->fullscreen) gtk_window_unfullscreen(GTK_WINDOW(p->window));
    else gtk_window_fullscreen(GTK_WINDOW(p->window));
}

static gboolean window_state(GtkWidget *widget, GdkEventWindowState *event, Player *p) {
    (void)widget;
    p->fullscreen = (event->new_window_state & GDK_WINDOW_STATE_FULLSCREEN) != 0;
    return FALSE;
}

static void volume(GtkRange *range, Player *p) {
    char value[32]; g_snprintf(value, sizeof(value), "%.0f", gtk_range_get_value(range));
    const char *args[] = {"set", "volume", value, NULL}; command(p, args);
}

static void subtitles(GtkToggleButton *button, Player *p) {
    if (p->syncing) return;
    const char *args[] = {"set", "sub-visibility", gtk_toggle_button_get_active(button) ? "yes" : "no", NULL};
    command(p, args);
}

static gboolean key(GtkWidget *widget, GdkEventKey *event, Player *p) {
    (void)widget;
    switch (event->keyval) {
    case GDK_KEY_space: pause_video(NULL, p); return TRUE;
    case GDK_KEY_Left: {
        const char *args[] = {"seek", "-10", "relative", NULL}; command(p, args); return TRUE;
    }
    case GDK_KEY_Right: {
        const char *args[] = {"seek", "10", "relative", NULL}; command(p, args); return TRUE;
    }
    case GDK_KEY_f: case GDK_KEY_F: fullscreen(NULL, p); return TRUE;
    case GDK_KEY_Escape:
        if (p->fullscreen) { gtk_window_unfullscreen(GTK_WINDOW(p->window)); return TRUE; }
        break;
    }
    return FALSE;
}

static gboolean tick(Player *p) {
    for (;;) {
        mpv_event *event = mpv_wait_event(p->player, 0);
        if (event->event_id == MPV_EVENT_NONE) break;
        if (event->event_id == MPV_EVENT_FILE_LOADED) fprintf(stderr, "Native video: file loaded\n");
        if (event->event_id == MPV_EVENT_END_FILE) {
            mpv_event_end_file *end = event->data;
            if (end->reason == MPV_END_FILE_REASON_ERROR) error_dialog(p, mpv_error_string(end->error));
        }
        if (event->event_id == MPV_EVENT_COMMAND_REPLY && event->error < 0)
            fprintf(stderr, "Native video command: %s\n", mpv_error_string(event->error));
    }
    if (p->renderer) gtk_gl_area_queue_render(GTK_GL_AREA(p->video));
    int paused = 0, visible = 1;
    mpv_get_property(p->player, "pause", MPV_FORMAT_FLAG, &paused);
    gtk_button_set_label(GTK_BUTTON(p->pause), paused ? "Play" : "Pause");
    if (mpv_get_property(p->player, "sub-visibility", MPV_FORMAT_FLAG, &visible) >= 0) {
        p->syncing = TRUE;
        gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(p->subtitles), visible);
        p->syncing = FALSE;
    }
    double position = 0, duration = 0;
    mpv_get_property(p->player, "time-pos", MPV_FORMAT_DOUBLE, &position);
    mpv_get_property(p->player, "duration", MPV_FORMAT_DOUBLE, &duration);
    char text[80];
    g_snprintf(text, sizeof(text), "%.0f / %.0f seconds", position, duration);
    gtk_label_set_text(GTK_LABEL(p->status), text);
    return G_SOURCE_CONTINUE;
}

static void destroy(GtkWidget *widget, Player *p) {
    (void)widget;
    if (p->timer) { g_source_remove(p->timer); p->timer = 0; }
    gtk_main_quit();
}

int main(int argc, char **argv) {
    if (argc > 2) { fprintf(stderr, "Usage: %s [video-file]\n", argv[0]); return 1; }
    if (!gtk_init_check(&argc, &argv)) { fprintf(stderr, "Open this player from your Linux desktop terminal.\n"); return 1; }
    Player p = {0};
    if (argc == 2) p.path = g_canonicalize_filename(argv[1], NULL);
    else {
        GtkWidget *chooser = gtk_file_chooser_dialog_new("Open video", NULL, GTK_FILE_CHOOSER_ACTION_OPEN,
            "Cancel", GTK_RESPONSE_CANCEL, "Open", GTK_RESPONSE_ACCEPT, NULL);
        if (gtk_dialog_run(GTK_DIALOG(chooser)) == GTK_RESPONSE_ACCEPT)
            p.path = gtk_file_chooser_get_filename(GTK_FILE_CHOOSER(chooser));
        gtk_widget_destroy(chooser);
        if (!p.path) return 0;
    }
    // GTK adopts the desktop locale; libmpv requires dot-based numeric parsing.
    setlocale(LC_NUMERIC, "C");
    p.player = mpv_create();
    if (!p.player) { g_free(p.path); return 1; }
    mpv_set_option_string(p.player, "config", "no");
    mpv_set_option_string(p.player, "vo", "libmpv");
    mpv_set_option_string(p.player, "idle", "yes");
    mpv_set_option_string(p.player, "keep-open", "yes");
    int result = mpv_initialize(p.player);
    if (result < 0) {
        fprintf(stderr, "libmpv: %s\n", mpv_error_string(result));
        mpv_terminate_destroy(p.player); g_free(p.path); return 1;
    }
    p.window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    char *name = g_path_get_basename(p.path);
    gtk_window_set_title(GTK_WINDOW(p.window), name); g_free(name);
    gtk_window_set_default_size(GTK_WINDOW(p.window), 1000, 650);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_container_add(GTK_CONTAINER(p.window), box);
    p.video = gtk_gl_area_new();
    gtk_gl_area_set_required_version(GTK_GL_AREA(p.video), 3, 2);
    gtk_gl_area_set_auto_render(GTK_GL_AREA(p.video), FALSE);
    gtk_widget_set_can_focus(p.video, TRUE);
    gtk_box_pack_start(GTK_BOX(box), p.video, TRUE, TRUE, 0);
    GtkWidget *controls = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    gtk_container_set_border_width(GTK_CONTAINER(controls), 8);
    gtk_box_pack_start(GTK_BOX(box), controls, FALSE, FALSE, 0);
    p.pause = gtk_button_new_with_label("Pause");
    gtk_box_pack_start(GTK_BOX(controls), p.pause, FALSE, FALSE, 0);
    g_signal_connect(p.pause, "clicked", G_CALLBACK(pause_video), &p);
    for (int i = 0; i < 2; i++) {
        GtkWidget *button = gtk_button_new_with_label(i ? "+10 sec" : "-10 sec");
        g_object_set_data(G_OBJECT(button), "seconds", i ? "10" : "-10");
        g_signal_connect(button, "clicked", G_CALLBACK(seek), &p);
        gtk_box_pack_start(GTK_BOX(controls), button, FALSE, FALSE, 0);
    }
    GtkWidget *full = gtk_button_new_with_label("Fullscreen");
    g_signal_connect(full, "clicked", G_CALLBACK(fullscreen), &p);
    gtk_box_pack_start(GTK_BOX(controls), full, FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(controls), gtk_label_new("Volume"), FALSE, FALSE, 0);
    GtkWidget *slider = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
    gtk_widget_set_size_request(slider, 140, -1); gtk_range_set_value(GTK_RANGE(slider), 100);
    gtk_scale_set_draw_value(GTK_SCALE(slider), FALSE);
    g_signal_connect(slider, "value-changed", G_CALLBACK(volume), &p);
    gtk_box_pack_start(GTK_BOX(controls), slider, FALSE, FALSE, 0);
    p.subtitles = gtk_check_button_new_with_label("Subtitles");
    gtk_toggle_button_set_active(GTK_TOGGLE_BUTTON(p.subtitles), TRUE);
    g_signal_connect(p.subtitles, "toggled", G_CALLBACK(subtitles), &p);
    gtk_box_pack_start(GTK_BOX(controls), p.subtitles, FALSE, FALSE, 0);
    p.status = gtk_label_new(""); gtk_box_pack_end(GTK_BOX(controls), p.status, FALSE, FALSE, 0);
    g_signal_connect(p.video, "realize", G_CALLBACK(realize), &p);
    g_signal_connect(p.video, "render", G_CALLBACK(render), &p);
    g_signal_connect(p.video, "unrealize", G_CALLBACK(unrealize), &p);
    g_signal_connect(p.window, "key-press-event", G_CALLBACK(key), &p);
    g_signal_connect(p.window, "window-state-event", G_CALLBACK(window_state), &p);
    g_signal_connect(p.window, "destroy", G_CALLBACK(destroy), &p);
    p.timer = g_timeout_add(16, (GSourceFunc)tick, &p);
    gtk_widget_show_all(p.window); gtk_widget_grab_focus(p.video);
    gtk_main();
    // The GL area's unrealize callback frees rendering before the player is destroyed.
    mpv_terminate_destroy(p.player); g_free(p.path);
    return 0;
}
