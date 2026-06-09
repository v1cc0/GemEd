# Vendored @google/model-viewer

This directory vendors the browser module used by GemEd's GLB preview and opt-in PNG snapshot capture adapter.

- Package: `@google/model-viewer`
- Version: `4.3.1`
- Source tarball: `https://registry.npmjs.org/@google/model-viewer/-/model-viewer-4.3.1.tgz`
- npm integrity: `sha512-GP+inXhAtY31E8rILVmByA6z8CZZjdlNajddppyI1/j1eIaSQiZcMRaUqTFe7+jv4mzRzwKIOiKBud0apiv+WQ==`
- Vendored file: `dist/model-viewer.min.js`
- SHA-256: `283b0672384614b4847636c306fc93fe4b1fcadc76d668b4e47f0ca76bcf033b`
- License: Apache-2.0, copied in `LICENSE`

GemEd loads this local module first for offline web/desktop bundles and falls back to the pinned CDN URL only if the local asset is missing.
