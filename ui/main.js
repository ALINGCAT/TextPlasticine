const { getCurrentWindow } = window.__TAURI__.window;
const { PhysicalPosition, LogicalPosition, LogicalSize } = window.__TAURI__.dpi;

const win = getCurrentWindow();
const editor = document.getElementById("editor");

let target = null;
let applying = false;

async function flush() {
  if (applying) return;
  applying = true;
  while (target) {
    const t = target;
    target = null;
    await t.apply();
  }
  applying = false;
}

function setupResizeHandles() {
  for (const handle of document.querySelectorAll(".handle")) {
    handle.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      window.__TAURI__.core.invoke("start_corner_resize").catch(() => {});
    });
  }
}

function setupCtrlDragMove() {
  document.addEventListener("pointerdown", async (e) => {
    if (e.button !== 0 || !e.ctrlKey) return;
    e.preventDefault();

    const captureEl = e.target;
    if (captureEl && captureEl.setPointerCapture) {
      try {
        captureEl.setPointerCapture(e.pointerId);
      } catch (_) {}
    }

    const scale = await win.scaleFactor();
    const startPos = await win.outerPosition();

    let accX = 0;
    let accY = 0;

    const onMove = (ev) => {
      accX += ev.movementX;
      accY += ev.movementY;
      const x = startPos.x + accX * scale;
      const y = startPos.y + accY * scale;
      target = {
        apply: async () => {
          await win.setPosition(new PhysicalPosition(x, y));
        },
      };
      flush();
    };

    const onUp = (ev) => {
      target = null;
      if (captureEl && captureEl.releasePointerCapture) {
        try {
          captureEl.releasePointerCapture(ev.pointerId);
        } catch (_) {}
      }
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };

    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  });
}

async function init() {
  const wa = screen.availWidth;
  const ha = screen.availHeight;

  let w, h, x, y;
  try {
    const cfg = await window.__TAURI__.core.invoke("get_startup_config");
    if (cfg.marked_rect && cfg.marked_rect.length === 4) {
      [x, y, w, h] = cfg.marked_rect.map(Math.round);
    } else {
      w = Math.round(wa * 0.5);
      h = Math.round(ha * 0.25);
      x = Math.round(wa * 0.55 - w / 2);
      y = Math.round(ha * 0.65 - h / 2);
      await window.__TAURI__.core.invoke("set_default_rect", { x, y, w, h });
    }
    await win.setSize(new LogicalSize(w, h));
    await win.setPosition(new LogicalPosition(x, y));
    if (!cfg.silent_start) {
      await win.show();
      editor.focus();
    }
  } catch (_) {
    w = Math.round(wa * 0.5);
    h = Math.round(ha * 0.25);
    await win.setSize(new LogicalSize(w, h));
    await win.setPosition(new LogicalPosition(Math.round(wa * 0.3), Math.round(ha * 0.525)));
    await win.show();
    editor.focus();
  }
}

window.addEventListener("focus", () => {
  editor.focus();
});

async function initDarkMode() {
  const { listen } = window.__TAURI__.event;
  const apply = (dark) => {
    document.body.classList.toggle("dark", !!dark);
  };
  try {
    await listen("dark-mode", (e) => apply(e.payload));
    apply(await window.__TAURI__.core.invoke("get_dark_mode"));
  } catch (_) {}
}

window.addEventListener("DOMContentLoaded", () => {
  setupResizeHandles();
  setupCtrlDragMove();
  initDarkMode();
  init();
});
