// SkiFree's miniquad plugin: the browser side of src/platform/web.rs.
//
// Loaded after miniquad's gl.js (which defines `wasm_memory`,
// `miniquad_add_plugin` and `load`) and before `load("skifree.wasm")`.
//
// Rust passes strings and bytes as (pointer, length) into its memory. Data
// going back to Rust is held here by `hold` (which returns its length, -1
// for none) and copied into a Rust buffer by `skifree_take`, never more
// than the buffer's length.

"use strict";

(function () {
    const canvas = document.querySelector("#glcanvas");
    const params = new URLSearchParams(window.location.search);
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();

    // ------------------------------------------------------------ memory

    let held = null;

    function hold(bytes) {
        if (bytes == null) {
            held = null;
            return -1;
        }
        held = bytes;
        return bytes.length;
    }

    // A fresh view each call: growing the wasm memory detaches old buffers.
    function view(ptr, len) {
        return new Uint8Array(wasm_memory.buffer, ptr >>> 0, len >>> 0);
    }

    function str(ptr, len) {
        return decoder.decode(view(ptr, len));
    }

    // ------------------------------------------------------------ sound
    //
    // sndPlaySoundA semantics: one sound at a time, a new one cuts off the
    // one playing. The WAVs (8-bit or 16-bit PCM) are decoded here rather
    // than by decodeAudioData, so every browser plays the same samples at
    // the file's own rate. Browsers start audio only after a user gesture:
    // the context is created on the first key press or click, and sounds
    // before that are dropped.

    const wavs = [];
    let audio = null;
    let current = null;

    function parseWav(bytes) {
        const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const tag = (o) => String.fromCharCode(bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]);
        if (bytes.length < 12 || tag(0) !== "RIFF" || tag(8) !== "WAVE") return null;
        let fmt = null;
        let data = null;
        for (let o = 12; o + 8 <= bytes.length;) {
            const len = dv.getUint32(o + 4, true);
            const body = o + 8;
            if (tag(o) === "fmt ") fmt = body;
            if (tag(o) === "data") data = bytes.subarray(body, Math.min(body + len, bytes.length));
            o = body + len + (len & 1);
        }
        if (fmt === null || data === null || dv.getUint16(fmt, true) !== 1) return null;
        const channels = dv.getUint16(fmt + 2, true);
        const rate = dv.getUint32(fmt + 4, true);
        const bits = dv.getUint16(fmt + 14, true);
        if (bits !== 8 && bits !== 16) return null;
        const frames = Math.floor(data.length / (channels * bits / 8));
        const samples = [];
        for (let c = 0; c < channels; c++) samples.push(new Float32Array(frames));
        const ddv = new DataView(data.buffer, data.byteOffset, data.byteLength);
        for (let i = 0; i < frames; i++) {
            for (let c = 0; c < channels; c++) {
                const k = i * channels + c;
                samples[c][i] = bits === 8 ? (data[k] - 128) / 128 : ddv.getInt16(k * 2, true) / 32768;
            }
        }
        return { rate, frames, samples, buffer: null };
    }

    function unlockAudio() {
        if (audio === null) {
            const Ctx = window.AudioContext || window.webkitAudioContext;
            if (!Ctx) return;
            audio = new Ctx();
        }
        if (audio.state !== "running") audio.resume();
    }

    function stopSound() {
        if (current !== null) {
            try {
                current.stop();
            } catch (e) {
                // already ended
            }
            current = null;
        }
    }

    function playSound(index) {
        stopSound();
        const w = wavs[index];
        if (!w || audio === null || audio.state !== "running") return;
        if (w.buffer === null) {
            w.buffer = audio.createBuffer(w.samples.length, w.frames, w.rate);
            w.samples.forEach((s, c) => w.buffer.copyToChannel(s, c));
        }
        const src = audio.createBufferSource();
        src.buffer = w.buffer;
        src.connect(audio.destination);
        src.onended = () => {
            if (current === src) current = null;
        };
        src.start();
        current = src;
    }

    // ------------------------------------------------------------ files
    //
    // The public build plays from the player's own files: ski32.exe (or
    // ski32.zip), optionally ski32sounds.exe/.zip and vgaoem.fon. Rust
    // extracts them; the page only collects them and, once Rust accepted
    // them, keeps them in IndexedDB so the next visit skips the drop box.
    // Nothing leaves the browser. Files are taken only by the drop box: to
    // change them, open the page with ?forget and drop them again.

    const MAX_FILE = 16 * 1024 * 1024;
    let offered = null; // { files: [{name, bytes}], cached: bool }

    function idb() {
        return new Promise((resolve, reject) => {
            const req = indexedDB.open("skifree", 1);
            req.onupgradeneeded = () => req.result.createObjectStore("files");
            req.onsuccess = () => resolve(req.result);
            req.onerror = () => reject(req.error);
        });
    }

    async function cache(mode, value) {
        try {
            const db = await idb();
            return await new Promise((resolve) => {
                const store = db.transaction("files", mode === "get" ? "readonly" : "readwrite").objectStore("files");
                const req = mode === "get" ? store.get("originals")
                    : mode === "put" ? store.put(value, "originals")
                        : store.delete("originals");
                req.onsuccess = () => resolve(req.result || null);
                req.onerror = () => resolve(null);
            });
        } catch (e) {
            return null;
        }
    }

    function dropBox() {
        return document.querySelector("#drop");
    }

    function showDrop(message) {
        const box = dropBox();
        if (!box) return;
        box.querySelector(".error").textContent = message || "";
        box.hidden = false;
    }

    function hideDrop() {
        const box = dropBox();
        if (box) box.hidden = true;
        canvas.focus();
    }

    async function readFiles(fileList) {
        const files = [];
        for (const f of fileList) {
            if (f.size > MAX_FILE) {
                showDrop(f.name + " is too big to be a SkiFree file.");
                return null;
            }
            files.push({ name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
        }
        return files;
    }

    async function startFiles() {
        // ?forget: drop the cached files (once: the parameter is taken out
        // of the address so a reload doesn't forget the next ones too).
        if (params.has("forget")) {
            await cache("delete");
            const url = new URL(window.location.href);
            url.searchParams.delete("forget");
            window.history.replaceState(null, "", url);
        }
        const cached = await cache("get");
        if (cached && cached.length) {
            offered = { files: cached, cached: true };
        } else {
            showDrop("");
        }
    }

    // Outside the drop box (the game running, in either build) a dragged
    // file shows the "not allowed" cursor and a drop does nothing: neither
    // the browser (which would open the file) nor miniquad (which would copy
    // it into the game's memory) gets it.
    function refuseDrops() {
        const refuse = (e) => {
            const box = dropBox();
            if (box && !box.hidden) return;
            e.preventDefault();
            e.stopPropagation();
            if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
        };
        window.addEventListener("dragover", refuse, true);
        window.addEventListener("drop", refuse, true);
    }

    function setupDropBox() {
        const box = dropBox();
        if (!box) return;
        const picker = box.querySelector("input[type=file]");
        const take = async (fileList) => {
            const files = await readFiles(fileList);
            if (files && files.length) offered = { files, cached: false };
        };
        box.addEventListener("dragover", (e) => {
            e.preventDefault();
            e.stopPropagation();
        });
        box.addEventListener("drop", (e) => {
            e.preventDefault();
            e.stopPropagation();
            take(e.dataTransfer.files);
        });
        if (picker) {
            picker.addEventListener("change", () => {
                take(picker.files);
                picker.value = "";
            });
        }
    }

    // ------------------------------------------------------------ capture
    //
    // Frame-capture mode (?replay=<scenario>&ticks=...): Rust hands over the
    // listed frames, raw RGBA top-down, for tools/web/capture_frames.mjs.

    window.skifreeCapture = {
        frames: {},
        done: false,
        base64(tick) {
            const f = this.frames[tick];
            if (!f) return null;
            let s = "";
            for (let i = 0; i < f.data.length; i += 0x8000) {
                s += String.fromCharCode.apply(null, f.data.subarray(i, i + 0x8000));
            }
            return btoa(s);
        },
    };

    // ------------------------------------------------------------ keys
    //
    // gl.js prevents the browser's default for Space, the arrows, F1-F10,
    // Tab, Backspace, ' and /. The game also uses these, which would
    // otherwise scroll or trigger buttons.
    const GAME_KEYS = new Set([
        "Home", "End", "PageUp", "PageDown", "Insert", "Enter", "NumpadEnter",
        "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4",
        "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9",
    ]);

    // Firefox's Quick Find opens on ' and / and takes the keyboard from the
    // game. gl.js means to block both, but it maps them to the key codes 222
    // and 189 and then checks 39 and 47, so they get through. Matched by
    // character, so any keyboard layout is covered (on some, / is Shift+7).
    const QUICK_FIND_KEYS = new Set(["'", "/"]);

    function onKeyDown(e) {
        unlockAudio();
        if (document.activeElement !== canvas) return;
        if (GAME_KEYS.has(e.code) || QUICK_FIND_KEYS.has(e.key)) e.preventDefault();
    }

    // ------------------------------------------------------------ plugin

    function register_plugin(importObject) {
        const env = importObject.env;

        env.skifree_take = (ptr, len) => {
            if (held !== null) {
                const n = Math.min(len >>> 0, held.length);
                view(ptr, n).set(held.subarray(0, n));
            }
            held = null;
        };

        env.skifree_set_title = (ptr, len) => {
            document.title = str(ptr, len);
        };
        env.skifree_is_fullscreen = () =>
            (document.fullscreenElement || document.webkitFullscreenElement) ? 1 : 0;

        env.skifree_storage_get = (ptr, len) => {
            try {
                const v = window.localStorage.getItem(str(ptr, len));
                return hold(v === null ? null : encoder.encode(v));
            } catch (e) {
                return hold(null);
            }
        };
        env.skifree_storage_set = (kptr, klen, vptr, vlen) => {
            try {
                window.localStorage.setItem(str(kptr, klen), str(vptr, vlen));
            } catch (e) {
                // storage blocked or full: the scores live for this visit only
            }
        };

        env.skifree_url_param = (ptr, len) => {
            const v = params.get(str(ptr, len));
            return hold(v === null ? null : encoder.encode(v));
        };

        env.skifree_audio_load = (index, ptr, len) => {
            wavs[index] = parseWav(view(ptr, len).slice());
        };
        env.skifree_audio_play = (index) => playSound(index);
        env.skifree_audio_stop = () => stopSound();

        env.skifree_set_favicon = (size, ptr, len) => {
            const c = document.createElement("canvas");
            c.width = c.height = size;
            const pixels = new Uint8ClampedArray(view(ptr, len));
            c.getContext("2d").putImageData(new ImageData(pixels, size, size), 0, 0);
            let link = document.querySelector('link[rel=icon][sizes="' + size + "x" + size + '"]');
            if (!link) {
                link = document.createElement("link");
                link.rel = "icon";
                link.sizes = size + "x" + size;
                document.head.appendChild(link);
            }
            link.href = c.toDataURL("image/png");
        };

        env.skifree_capture_frame = (tick, w, h, ptr, len) => {
            window.skifreeCapture.frames[tick] = { w, h, data: view(ptr, len).slice() };
        };
        env.skifree_capture_done = () => {
            window.skifreeCapture.done = true;
        };

        env.skifree_request_files = () => {
            startFiles();
        };
        env.skifree_file_count = () => (offered ? offered.files.length : -1);
        env.skifree_file_name = (i) => hold(encoder.encode(offered.files[i].name));
        env.skifree_file_bytes = (i) => hold(offered.files[i].bytes);
        env.skifree_assets_result = (ok, ptr, len) => {
            const message = str(ptr, len);
            const done = offered;
            offered = null;
            if (ok) {
                if (!done.cached) cache("put", done.files);
                hideDrop();
            } else {
                if (done.cached) cache("delete");
                showDrop(message);
            }
        };
    }

    function on_init() {
        refuseDrops();
        setupDropBox();
        window.addEventListener("keydown", onKeyDown, true);
        window.addEventListener("pointerdown", unlockAudio, true);
        window.addEventListener("touchend", unlockAudio, true);
    }

    // Version 2: skifree_take takes the buffer length.
    miniquad_add_plugin({ register_plugin, on_init, name: "skifree", version: 2 });
})();
