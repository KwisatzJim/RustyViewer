/* Rust owns image pixels and disk writes. This file owns presentation and interaction. */
const $ = (id) => document.getElementById(id);
const icons = {
  folder: '<path d="M3 7h6l2 2h10v11H3z"/><path d="M3 7V4h6l2 3h8v2"/>',
  export: '<path d="M12 15V3m-4 4 4-4 4 4M5 13v7h14v-7"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5"/>',
  undo: '<path d="m8 4-5 5 5 5M3 9h11a6 6 0 0 1 0 12"/>',
  redo: '<path d="m16 4 5 5-5 5m5-5H10a6 6 0 0 0 0 12"/>',
  rotate: '<path d="M3 10a9 9 0 1 1 2 8M3 4v6h6"/>',
  crop: '<path d="M6 2v16h16M2 6h16v16"/>',
  resize:
    '<path d="M4 9V4h5m6 0h5v5m0 6v5h-5M9 20H4v-5M4 4l6 6m10-6-6 6m6 10-6-6M4 20l6-6"/>',
  fit: '<path d="M3 8V3h5m8 0h5v5m0 8v5h-5M8 21H3v-5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  image:
    '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="16" cy="8" r="2"/><path d="m3 17 6-7 6 8 3-4 3 3"/>',
  reset: '<path d="M3 10a9 9 0 1 1 2 8M3 4v6h6m3-4v6l4 2"/>',
  flip: '<path d="M12 3v18M3 17l5-10v10H3Zm18 0L16 7v10h5Z"/>',
  contrast: '<circle cx="12" cy="12" r="9"/><path d="M12 3v18"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1 1m12 12 1 1M5 19l1-1M18 6l1-1"/>',
  shield:
    '<path d="m12 2 8 3v6c0 6-8 11-8 11S4 17 4 11V5l8-3Z"/><path d="m8 12 3 3 5-6"/>',
};
const icon = (name) =>
  `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${icons[name] || icons.image}</svg>`;
document.querySelectorAll("[data-icon]").forEach((el) => {
  el.innerHTML = icon(el.dataset.icon);
});
const tauri = window.__TAURI__;
const invoke = (command, args = {}) =>
  tauri
    ? tauri.core.invoke(command, args)
    : Promise.reject(new Error("Open the desktop app to use image tools."));
const mac = /Mac|iPhone|iPad/.test(navigator.platform);
const mod = mac ? "⌘" : "Ctrl+";
$("open-shortcut").textContent = `${mod} O`;
let doc = null,
  working = false,
  batchRunning = false,
  batchFiles = [],
  zoom = 1,
  fit = true,
  pan = { x: 0, y: 0 },
  cropMode = false,
  cropRect = null,
  pointer = null;
let toastTimer;
function notify(message, error = false) {
  clearTimeout(toastTimer);
  (document.querySelector("dialog[open]") || document.body).append($("toast"));
  $("toast-text").textContent = String(message);
  $("toast").classList.toggle("error", error);
  $("toast").hidden = false;
  if (!error) toastTimer = setTimeout(() => ($("toast").hidden = true), 6000);
}
function controls() {
  document
    .querySelectorAll("[data-image]")
    .forEach((el) => (el.disabled = !doc || working));
  $("adjustments").disabled = !doc || working;
  $("open").disabled = working;
  $("welcome-open").disabled = working;
  $("undo").disabled = !doc?.can_undo || working;
  $("redo").disabled = !doc?.can_redo || working;
  $("previous").disabled = !doc || doc.siblings.length < 2 || working;
  $("next").disabled = $("previous").disabled;
  $("reset").disabled = !doc || working;
  $("paste-empty").hidden = !!doc;
  $("paste-empty").disabled = working;
}
async function run(label, action) {
  if (working) {
    notify("Please wait for the current image operation.");
    return;
  }
  working = true;
  controls();
  $("busy-label").textContent = label;
  $("busy").hidden = false;
  try {
    return await action();
  } catch (e) {
    notify(e.message || String(e), true);
  } finally {
    working = false;
    $("busy").hidden = true;
    controls();
  }
}
function syncSliders() {
  for (const key of [
    "brightness",
    "contrast",
    "saturation",
    "gamma",
    "red",
    "green",
    "blue",
  ]) {
    const el = $(`slider-${key}`);
    el.value = doc?.adjustments[key] ?? (key === "gamma" ? 1 : 0);
    sliderLabel(key);
  }
}
function sliderLabel(key) {
  const value = Number($(`slider-${key}`).value);
  $(`value-${key}`).textContent =
    key === "gamma"
      ? value.toFixed(2)
      : `${value > 0 ? "+" : ""}${Math.round(value * 100)}`;
}
for (const [key, label, min, max, step] of [
  ["brightness", "Brightness", -1, 1, 0.01],
  ["contrast", "Contrast", -1, 1, 0.01],
  ["saturation", "Saturation", -1, 1, 0.01],
  ["gamma", "Gamma", 0.1, 3, 0.01],
  ["red", "R", -1, 1, 0.01],
  ["green", "G", -1, 1, 0.01],
  ["blue", "B", -1, 1, 0.01],
]) {
  const tint = ["red", "green", "blue"].includes(key),
    row = document.createElement("div");
  row.className = tint ? "tint-row" : "slider-row";
  const input = `<input id="slider-${key}" aria-label="${tint ? key + " tint" : label}" type="range" min="${min}" max="${max}" step="${step}" value="${key === "gamma" ? 1 : 0}">`;
  row.innerHTML = tint
    ? `<label for="slider-${key}">${label}</label>${input}<output id="value-${key}"></output>`
    : `<div class="slider-label"><label for="slider-${key}">${label}</label><output id="value-${key}"></output></div>${input}`;
  $(tint ? "tints" : "sliders").append(row);
  $(`slider-${key}`).oninput = () => sliderLabel(key);
  $(`slider-${key}`).onchange = () => {
    const adjustments = Object.fromEntries(
      [
        "brightness",
        "contrast",
        "saturation",
        "gamma",
        "red",
        "green",
        "blue",
      ].map((k) => [k, Number($(`slider-${k}`).value)]),
    );
    run("Applying adjustments…", async () =>
      setDocument(await invoke("adjust_image", { adjustments })),
    ).then(syncSliders);
  };
}
function setDocument(value, resetView = false) {
  if (!value) return;
  const dimensionsChanged =
    doc && (doc.width !== value.width || doc.height !== value.height);
  doc = value;
  $("welcome").hidden = true;
  $("image-stage").hidden = false;
  $("image").src = doc.preview;
  $("image").alt = doc.name;
  $("document-name").textContent = doc.name;
  $("document-name").title = doc.path || doc.name;
  $("dirty").hidden = !doc.dirty;
  $("image-info").textContent =
    `${doc.width.toLocaleString()} × ${doc.height.toLocaleString()} px${doc.bytes ? " · " + (doc.bytes / 1024 / 1024).toFixed(2) + " MB" : ""}`;
  $("position").textContent = doc.siblings.length
    ? `${doc.index + 1} / ${doc.siblings.length}`
    : "1 / 1";
  $("file-count").textContent = doc.siblings.length;
  $("folder-name").textContent = doc.path
    ? doc.path.split(/[\\/]/).slice(-2, -1)[0]
    : "Clipboard";
  $("file-list").replaceChildren();
  doc.siblings.forEach((path, index) => {
    const button = document.createElement("button");
    button.className = "file-row" + (index === doc.index ? " selected" : "");
    button.title = path;
    button.innerHTML = icon("image");
    const name = document.createElement("span");
    name.textContent = path.split(/[\\/]/).pop();
    button.append(name);
    button.onclick = () => {
      if (index !== doc.index) openPath(path);
    };
    $("file-list").append(button);
  });
  $("file-list")
    .querySelector(".selected")
    ?.scrollIntoView({ block: "nearest" });
  if (resetView || dimensionsChanged) {
    fit = true;
    pan = { x: 0, y: 0 };
    setCrop(false);
  }
  syncSliders();
  layoutImage();
  controls();
}
function layoutImage() {
  if (!doc) return;
  const width = $("viewport").clientWidth,
    height = $("viewport").clientHeight;
  if (fit)
    zoom = Math.min((width - 48) / doc.width, (height - 48) / doc.height, 1);
  const stage = $("image-stage");
  stage.style.width = `${doc.width * zoom}px`;
  stage.style.height = `${doc.height * zoom}px`;
  stage.style.left = `${(width - doc.width * zoom) / 2 + pan.x}px`;
  stage.style.top = `${(height - doc.height * zoom) / 2 + pan.y}px`;
  $("zoom-label").textContent = `${Math.round(zoom * 100)}%`;
  $("fit").classList.toggle("active", fit);
  renderCrop();
}
function changeZoom(value) {
  if (!doc) return;
  fit = false;
  zoom = Math.max(0.01, Math.min(32, value));
  layoutImage();
}
function setCrop(enabled) {
  cropMode = enabled;
  if (enabled) $("viewport").focus();
  cropRect = null;
  $("selection").hidden = true;
  $("crop-hint").hidden = !enabled;
  $("viewport").classList.toggle("cropping", enabled);
  $("crop").classList.toggle("active", enabled);
}
function renderCrop() {
  if (!cropRect) return;
  const el = $("selection");
  el.hidden = false;
  el.style.left = `${cropRect.x * zoom}px`;
  el.style.top = `${cropRect.y * zoom}px`;
  el.style.width = `${cropRect.w * zoom}px`;
  el.style.height = `${cropRect.h * zoom}px`;
}
function imagePoint(event) {
  const rect = $("image-stage").getBoundingClientRect();
  return {
    x: Math.max(0, Math.min(doc.width, (event.clientX - rect.left) / zoom)),
    y: Math.max(0, Math.min(doc.height, (event.clientY - rect.top) / zoom)),
  };
}
$("image-stage").onpointerdown = (e) => {
  if (!doc || working || e.button !== 0) return;
  $("image-stage").setPointerCapture(e.pointerId);
  pointer = {
    start: imagePoint(e),
    x: e.clientX,
    y: e.clientY,
    pan: { ...pan },
  };
  if (cropMode) {
    cropRect = null;
    $("selection").hidden = true;
  }
};
$("image-stage").onpointermove = (e) => {
  if (!pointer) return;
  if (cropMode) {
    const p = imagePoint(e);
    cropRect = {
      x: Math.floor(Math.min(p.x, pointer.start.x)),
      y: Math.floor(Math.min(p.y, pointer.start.y)),
      w: Math.floor(Math.abs(p.x - pointer.start.x)),
      h: Math.floor(Math.abs(p.y - pointer.start.y)),
    };
    renderCrop();
  } else {
    pan = {
      x: pointer.pan.x + e.clientX - pointer.x,
      y: pointer.pan.y + e.clientY - pointer.y,
    };
    layoutImage();
  }
};
$("image-stage").onpointerup = () => (pointer = null);
$("image-stage").onpointercancel = () => (pointer = null);
$("viewport").addEventListener(
  "wheel",
  (e) => {
    if (!doc) return;
    e.preventDefault();
    changeZoom(zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
  },
  { passive: false },
);
$("viewport").ondblclick = () => {
  if (doc) {
    fit = true;
    pan = { x: 0, y: 0 };
    layoutImage();
  }
};
new ResizeObserver(layoutImage).observe($("viewport"));
async function discardChanges() {
  return (
    !doc?.dirty ||
    (await tauri.dialog.confirm(
      "This image has changes that have not been exported. Discard them?",
      {
        title: "Unsaved changes",
        kind: "warning",
        okLabel: "Discard changes",
        cancelLabel: "Keep editing",
      },
    ))
  );
}
async function openPath(path) {
  return run("Opening image…", async () => {
    if (await discardChanges())
      setDocument(await invoke("open_image", { path, discard: true }), true);
  });
}
async function openDialog() {
  return run("Choose an image…", async () => {
    const path = await tauri.dialog.open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: [
            "png",
            "jpg",
            "jpeg",
            "webp",
            "gif",
            "bmp",
            "tif",
            "tiff",
            "ico",
          ],
        },
      ],
    });
    if (path && (await discardChanges()))
      setDocument(await invoke("open_image", { path, discard: true }), true);
  });
}
async function exportImage() {
  if (!doc) return;
  return run("Exporting image…", async () => {
    const stem = doc.name.replace(/\.[^.]+$/, "");
    const path = await tauri.dialog.save({
      defaultPath: `${stem}-edited.png`,
      filters: [
        { name: "PNG", extensions: ["png"] },
        { name: "JPEG", extensions: ["jpg", "jpeg"] },
        { name: "WebP", extensions: ["webp"] },
        { name: "BMP", extensions: ["bmp"] },
        { name: "TIFF", extensions: ["tiff", "tif"] },
      ],
    });
    if (!path) return;
    await invoke("export_image", { path });
    doc.dirty = false;
    $("dirty").hidden = true;
    notify(`Exported to ${path}`);
  });
}
function edit(action, args = []) {
  if (!doc) return;
  return run("Updating image…", async () => {
    setDocument(await invoke("edit_image", { action, args }));
    setCrop(false);
  });
}
function navigate(delta) {
  if (!doc || doc.siblings.length < 2) return;
  openPath(
    doc.siblings[
      (doc.index + delta + doc.siblings.length) % doc.siblings.length
    ],
  );
}
async function paste() {
  return run("Pasting image…", async () => {
    if (await discardChanges())
      setDocument(await invoke("paste_image", { discard: true }), true);
  });
}
$("open").onclick = openDialog;
$("welcome-open").onclick = openDialog;
$("export").onclick = exportImage;
$("undo").onclick = () => edit("undo");
$("redo").onclick = () => edit("redo");
$("previous").onclick = () => navigate(-1);
$("next").onclick = () => navigate(1);
$("zoom-in").onclick = () => changeZoom(zoom * 1.25);
$("zoom-out").onclick = () => changeZoom(zoom / 1.25);
$("actual").onclick = () => {
  pan = { x: 0, y: 0 };
  changeZoom(1);
};
$("fit").onclick = () => {
  fit = true;
  pan = { x: 0, y: 0 };
  layoutImage();
};
$("reset").onclick = () =>
  run("Resetting adjustments…", async () =>
    setDocument(
      await invoke("adjust_image", {
        adjustments: {
          brightness: 0,
          contrast: 0,
          saturation: 0,
          gamma: 1,
          red: 0,
          green: 0,
          blue: 0,
        },
      }),
    ),
  );
$("crop").onclick = () => setCrop(!cropMode);
$("apply-crop").onclick = () => {
  if (cropRect && cropRect.w && cropRect.h)
    edit("crop", [cropRect.x, cropRect.y, cropRect.w, cropRect.h]);
  else notify("Drag over the image to select a crop area.");
};
$("cancel-crop").onclick = () => setCrop(false);
$("copy").onclick = () =>
  run("Copying image…", async () => {
    await invoke("copy_image");
    notify("Image copied to clipboard.");
  });
$("paste").onclick = paste;
$("paste-empty").onclick = paste;
$("toast-close").onclick = () => ($("toast").hidden = true);
document
  .querySelectorAll("[data-action]")
  .forEach((el) => (el.onclick = () => edit(el.dataset.action)));
document
  .querySelectorAll("[data-close]")
  .forEach((el) => (el.onclick = () => $(el.dataset.close).close()));
$("resize").onclick = () => {
  if (!doc) return;
  $("resize-width").value = doc.width;
  $("resize-height").value = doc.height;
  $("resize-original").textContent =
    `Current size: ${doc.width} × ${doc.height} pixels`;
  $("resize-dialog").showModal();
};
$("resize-width").oninput = () => {
  if ($("resize-lock").checked && doc)
    $("resize-height").value = Math.max(
      1,
      Math.round((Number($("resize-width").value) * doc.height) / doc.width),
    );
};
$("resize-height").oninput = () => {
  if ($("resize-lock").checked && doc)
    $("resize-width").value = Math.max(
      1,
      Math.round((Number($("resize-height").value) * doc.width) / doc.height),
    );
};
$("resize-form").onsubmit = async (e) => {
  e.preventDefault();
  const w = Number($("resize-width").value),
    h = Number($("resize-height").value);
  if (
    !Number.isInteger(w) ||
    !Number.isInteger(h) ||
    w < 1 ||
    h < 1 ||
    w * h > 80000000
  ) {
    notify("Use positive whole dimensions up to 80 megapixels.", true);
    return;
  }
  $("resize-dialog").close();
  await edit("resize", [w, h]);
};
function renderBatch() {
  $("batch-count").textContent = batchFiles.length;
  $("batch-files").replaceChildren();
  if (!batchFiles.length) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = "Add images or drop files into this window.";
    $("batch-files").append(p);
  }
  batchFiles.forEach((path, i) => {
    const row = document.createElement("div");
    row.className = "batch-file";
    row.title = path;
    const name = document.createElement("span");
    name.textContent = path.split(/[\\/]/).pop();
    const remove = document.createElement("button");
    remove.textContent = "×";
    remove.setAttribute("aria-label", `Remove ${name.textContent}`);
    remove.disabled = batchRunning;
    remove.onclick = () => {
      batchFiles.splice(i, 1);
      renderBatch();
    };
    row.append(name, remove);
    $("batch-files").append(row);
  });
  $("batch-run").disabled =
    batchRunning || !batchFiles.length || !$("batch-directory").value;
}
function addBatch(paths) {
  if (batchRunning) return;
  batchFiles = [...new Set([...batchFiles, ...paths])];
  renderBatch();
}
$("batch").onclick = () => {
  $("batch-dialog").showModal();
  renderBatch();
};
$("batch-top").onclick = () => $("batch").click();
$("batch-add").onclick = async () => {
  try {
    const paths = await tauri.dialog.open({
      multiple: true,
      filters: [
        {
          name: "Images",
          extensions: [
            "png",
            "jpg",
            "jpeg",
            "webp",
            "gif",
            "bmp",
            "tif",
            "tiff",
            "ico",
          ],
        },
      ],
    });
    if (paths) addBatch(paths);
  } catch (e) {
    notify(String(e), true);
  }
};
$("batch-clear").onclick = () => {
  batchFiles = [];
  renderBatch();
};
$("batch-folder").onclick = async () => {
  try {
    const path = await tauri.dialog.open({
      directory: true,
      defaultPath: $("batch-directory").value || undefined,
    });
    if (path) {
      $("batch-directory").value = path;
      renderBatch();
    }
  } catch (e) {
    notify(String(e), true);
  }
};
function batchProgress(result) {
  $("batch-progress").hidden = false;
  $("batch-progress").max = result.total;
  $("batch-progress").value = result.completed;
  $("batch-result").textContent =
    `${result.completed} / ${result.total} processed · ${result.written} exported · ${result.errors.length} skipped or failed` +
    (result.errors.length ? "\n" + result.errors.join("\n") : "");
}
$("batch-run").onclick = async () => {
  if (batchRunning) return;
  const options = {
    files: [...batchFiles],
    directory: $("batch-directory").value,
    format: $("batch-format").value,
    resize: $("batch-resize").checked
      ? [Number($("batch-width").value), Number($("batch-height").value)]
      : null,
    keep_aspect: $("batch-aspect").checked,
    rotation: Number($("batch-rotation").value),
    grayscale: $("batch-gray").checked,
    invert: $("batch-invert").checked,
    auto: $("batch-auto").checked,
  };
  if (
    (options.resize &&
      options.resize.some((n) => !Number.isInteger(n) || n < 1)) ||
    (options.resize && options.resize[0] * options.resize[1] > 80000000)
  ) {
    notify("Use positive whole dimensions up to 80 megapixels.", true);
    return;
  }
  batchRunning = true;
  $("batch-dialog")
    .querySelectorAll("button,input,select")
    .forEach((el) => (el.disabled = true));
  $("batch-result").textContent = "Starting…";
  try {
    const result = await invoke("run_batch", { options });
    batchProgress(result);
    notify(
      `Batch finished: ${result.written} exported, ${result.errors.length} skipped or failed.`,
      result.errors.length > 0,
    );
  } catch (e) {
    $("batch-result").textContent = String(e);
    notify(String(e), true);
  } finally {
    batchRunning = false;
    $("batch-dialog")
      .querySelectorAll("button,input,select")
      .forEach((el) => (el.disabled = false));
    renderBatch();
  }
};
$("batch-dialog").addEventListener("cancel", (e) => {
  if (batchRunning) e.preventDefault();
});
const shortcuts = [
  ["Open image", `${mod} O`],
  ["Export image", `${mod} S`],
  ["Undo / redo", `${mod} Z / ${mod} Shift Z`],
  ["Copy / paste image", `${mod} C / ${mod} V`],
  ["Previous / next image", "← / →"],
  ["Rotate left / right", "L / R"],
  ["Flip horizontal / vertical", "H / V"],
  ["Grayscale / invert", "G / I"],
  ["Auto-adjust", "Shift U"],
  ["Crop selection / apply", "C / Enter"],
  ["Fit / actual size", "F / 1"],
  ["Zoom in / out", "+ / −"],
  ["Pan", "Drag image"],
  ["Batch studio", `${mod} B`],
];
shortcuts.forEach(([name, key]) => {
  const row = document.createElement("div");
  const label = document.createElement("span");
  label.textContent = name;
  const kbd = document.createElement("kbd");
  kbd.textContent = key;
  row.append(label, kbd);
  $("shortcut-list").append(row);
});
$("help").onclick = () => $("help-dialog").showModal();
document.addEventListener("keydown", (e) => {
  // Let focused buttons keep their native Enter/Space activation.
  if (e.target.closest("button") && (e.key === "Enter" || e.key === " "))
    return;
  if (
    document.querySelector("dialog[open]") ||
    /INPUT|TEXTAREA|SELECT/.test(e.target.tagName) ||
    working
  )
    return;
  const key = e.key.toLowerCase(),
    modifier = e.metaKey || e.ctrlKey;
  if (modifier) {
    const action = {
      o: openDialog,
      s: exportImage,
      z: () => edit(e.shiftKey ? "redo" : "undo"),
      c: () => $("copy").click(),
      v: paste,
      b: () => $("batch").click(),
    }[key];
    if (action) {
      e.preventDefault();
      action();
    }
    return;
  }
  const action = {
    arrowleft: () => navigate(-1),
    arrowright: () => navigate(1),
    r: () => edit("rotate_right"),
    l: () => edit("rotate_left"),
    h: () => edit("flip_horizontal"),
    v: () => edit("flip_vertical"),
    g: () => edit("grayscale"),
    i: () => edit("invert"),
    u: () => {
      if (e.shiftKey) edit("auto");
    },
    f: () => $("fit").click(),
    1: () => $("actual").click(),
    "+": () => $("zoom-in").click(),
    "=": () => $("zoom-in").click(),
    "-": () => $("zoom-out").click(),
    c: () => {
      if (doc) setCrop(!cropMode);
    },
    escape: () => setCrop(false),
    enter: () => {
      if (cropRect && cropRect.w && cropRect.h)
        edit("crop", [cropRect.x, cropRect.y, cropRect.w, cropRect.h]);
    },
    "?": () => $("help").click(),
  }[key];
  if (action) {
    e.preventDefault();
    action();
  }
});
async function initialize() {
  syncSliders();
  controls();
  if (!tauri) {
    $("status").textContent =
      "Interface preview · open the desktop app for image tools";
    return;
  }
  await tauri.event.listen("batch-progress", (event) =>
    batchProgress(event.payload),
  );
  await tauri.event.listen("open-file", (event) => openPath(event.payload));
  await tauri.webview.getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    $("drop-overlay").hidden =
      payload.type !== "over" && payload.type !== "enter";
    if (payload.type === "drop") {
      if ($("batch-dialog").open) addBatch(payload.paths);
      else if (payload.paths.length) openPath(payload.paths[0]);
    }
  });
  await tauri.window.getCurrentWindow().onCloseRequested(async (event) => {
    event.preventDefault();
    if (working || batchRunning) {
      notify("Please wait for the current operation to finish.");
      return;
    }
    if (await discardChanges()) await invoke("quit_app");
  });
  await tauri.event.listen("request-quit", async () => {
    if (working || batchRunning) {
      notify("Please wait for the current operation to finish.");
      return;
    }
    if (await discardChanges()) await invoke("quit_app");
  });
  const [path, directory] = await invoke("startup");
  $("batch-directory").value = directory;
  if (path) await openPath(path);
}
initialize().catch((e) =>
  notify(`Could not initialize the desktop interface: ${e}`, true),
);
