// JS shim to expose Tauri's convertFileSrc to WASM
// This provides a reliable way to convert file paths to asset:// URLs

// Import convertFileSrc from Tauri v2 API
const tauriCore = window.__TAURI__?.core;

/**
 * Convert a file path to an asset:// URL that can be loaded by the webview
 * @param {string} path - Absolute file path
 * @returns {string} - Asset protocol URL
 */
export function convertFileSrc(path) {
  if (tauriCore && typeof tauriCore.convertFileSrc === 'function') {
    return tauriCore.convertFileSrc(path);
  }
  
  // Fallback: return path as-is (won't work, but prevents crashes)
  console.warn('Tauri convertFileSrc not available, returning raw path');
  return path;
}

// Expose to window for easy access from WASM
window.__APP__convertFileSrc = convertFileSrc;
