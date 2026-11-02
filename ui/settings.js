const { invoke } = window.__TAURI__.core;

const msg = document.getElementById("msg");
const resetBtn = document.getElementById("reset");
const darkToggle = document.getElementById("dark-toggle");
const silentToggle = document.getElementById("silent-toggle");
const autostartToggle = document.getElementById("autostart-toggle");

const DISPLAY_MAP = {
  control: "Ctrl",
  ctrl: "Ctrl",
  shift: "Shift",
  alt: "Alt",
  super: "Win",
  meta: "Win",
  win: "Win",
  space: "Space",
  backquote: "`",
};

const FIELDS = [
  { kind: "toggle", el: document.getElementById("hotkey-input") },
  { kind: "mark", el: document.getElementById("mark-input") },
  { kind: "restore", el: document.getElementById("restore-input") },
];

function pretty(raw) {
  return raw
    .split("+")
    .map((p) => {
      const lower = p.toLowerCase();
      if (DISPLAY_MAP[lower]) return DISPLAY_MAP[lower];
      const keyMatch = lower.match(/^key([a-z])$/);
      if (keyMatch) return keyMatch[1].toUpperCase();
      const digitMatch = lower.match(/^digit([0-9])$/);
      if (digitMatch) return digitMatch[1];
      return p.length === 1 ? p.toUpperCase() : p.charAt(0).toUpperCase() + p.slice(1);
    })
    .join(" + ");
}

async function applyShortcut(kind, raw) {
  const field = FIELDS.find((f) => f.kind === kind);
  field.el.value = pretty(raw);
  msg.style.color = "";
  try {
    await invoke("set_shortcut", { kind, hotkey: raw });
    msg.style.color = "#15803d";
    msg.textContent = "已生效";
  } catch (err) {
    msg.style.color = "#b91c1c";
    msg.textContent = String(err);
  }
}

function normalizeKey(e) {
  const k = e.key;
  if (k === " " || k === "Spacebar") return "Space";
  if (k === "`" || k === "~") return "`";
  if (/^[a-z]$/i.test(k)) return k.toLowerCase();
  if (/^[0-9]$/.test(k)) return k;
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(k)) return k;
  const map = {
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    Enter: "Enter",
    Tab: "Tab",
    Escape: "Escape",
    Backspace: "Backspace",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
  };
  return map[k] || null;
}

for (const field of FIELDS) {
  field.el.addEventListener("focus", () => {
    invoke("suspend_hotkeys").catch(() => {});
  });
  field.el.addEventListener("blur", () => {
    invoke("restore_hotkeys").catch(() => {});
  });
  field.el.addEventListener("keydown", (e) => {
    e.preventDefault();
    e.stopPropagation();

    const mods = [];
    if (e.ctrlKey) mods.push("ctrl");
    if (e.altKey) mods.push("alt");
    if (e.shiftKey) mods.push("shift");
    if (e.metaKey) mods.push("super");

    const key = normalizeKey(e);
    if (!key) {
      msg.style.color = "#b91c1c";
      msg.textContent = "不支持该按键，请换一个";
      return;
    }
    if (mods.length === 0) {
      msg.style.color = "#b91c1c";
      msg.textContent = "请至少包含一个修饰键（Ctrl / Alt / Shift / Win）";
      return;
    }

    applyShortcut(field.kind, [...mods, key].join("+"));
  });
}

resetBtn.addEventListener("click", () => {
  applyShortcut("toggle", "ctrl+space");
  applyShortcut("mark", "alt+m");
  applyShortcut("restore", "alt+`");
});

darkToggle.addEventListener("change", async () => {
  try {
    await invoke("set_dark_mode", { dark: darkToggle.checked });
    msg.style.color = "#15803d";
    msg.textContent = "已生效";
  } catch (err) {
    msg.style.color = "#b91c1c";
    msg.textContent = String(err);
  }
});

silentToggle.addEventListener("change", async () => {
  try {
    await invoke("set_silent_start", { silent: silentToggle.checked });
    msg.style.color = "#15803d";
    msg.textContent = "已生效";
  } catch (err) {
    msg.style.color = "#b91c1c";
    msg.textContent = String(err);
  }
});

autostartToggle.addEventListener("change", async () => {
  try {
    await invoke("set_autostart", { enable: autostartToggle.checked });
    msg.style.color = "#15803d";
    msg.textContent = "已生效";
  } catch (err) {
    msg.style.color = "#b91c1c";
    msg.textContent = String(err);
  }
});

async function init() {
  for (const field of FIELDS) {
    try {
      const h = await invoke("get_shortcut", { kind: field.kind });
      field.el.value = pretty(h);
    } catch (err) {
      msg.style.color = "#b91c1c";
      msg.textContent = String(err);
    }
  }
  try {
    darkToggle.checked = await invoke("get_dark_mode");
    silentToggle.checked = await invoke("get_silent_start");
    autostartToggle.checked = await invoke("get_autostart");
  } catch (_) {}
}

init();
