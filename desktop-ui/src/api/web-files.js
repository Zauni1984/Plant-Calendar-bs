// Browser stand-ins for the desktop's native file dialogs.
//
// main.js uses a two-step pattern everywhere: ask a dialog for a path, then
// pass that path to a Rust command that reads or writes the file. Browsers
// have no paths, so the dialogs hand back opaque tokens instead:
//   save → "web-download:<filename>"; writing to it triggers a download
//   open → "web-upload:<n>";          reading it returns the chosen file's text
// The call sites stay unchanged.

const DOWNLOAD_PREFIX = "web-download:";
const UPLOAD_PREFIX = "web-upload:";
const uploads = new Map();
let uploadSeq = 0;

function baseName(path) {
  return String(path || "export.txt").split(/[\\/]/).pop() || "export.txt";
}

export function webSaveDialog(options = {}) {
  return Promise.resolve(DOWNLOAD_PREFIX + baseName(options.defaultPath));
}

export function triggerDownload(path, content, mime = "text/plain;charset=utf-8") {
  const name = String(path).startsWith(DOWNLOAD_PREFIX)
    ? String(path).slice(DOWNLOAD_PREFIX.length)
    : baseName(path);
  const url = URL.createObjectURL(new Blob([content], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoke on the next tick: some browsers start the download asynchronously.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

function acceptFromFilters(filters) {
  const exts = (filters || []).flatMap((f) => f.extensions || []);
  return exts.length ? exts.map((e) => `.${e}`).join(",") : "";
}

export function webOpenDialog(options = {}) {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = acceptFromFilters(options.filters);
    input.style.display = "none";
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(value);
    };
    input.addEventListener("change", () => {
      const file = input.files && input.files[0];
      if (!file) return finish(null);
      const token = `${UPLOAD_PREFIX}${++uploadSeq}`;
      uploads.set(token, file);
      finish(token);
    });
    // Fired when the picker is dismissed (Chromium 113+, Safari 16.4+, Firefox 91+).
    input.addEventListener("cancel", () => finish(null));
    document.body.appendChild(input);
    input.click();
  });
}

export async function takeUpload(token) {
  const file = uploads.get(token);
  if (!file) throw "Die ausgewählte Datei ist nicht mehr verfügbar. Bitte erneut auswählen.";
  uploads.delete(token);
  return file.text();
}
