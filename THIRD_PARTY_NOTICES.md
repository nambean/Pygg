# Third-party notices

Pygg is MIT-licensed (LICENSE). Parts of it follow or port code from the projects below, under their licences.

## 9router

The Muse web-session recipe (`apps/desktop/src-tauri/src/websession/mod.rs`, `RECIPES`: the page's selectors, the
approval card and its deny button, the sign-in and quota words, the new-chat address, the polling and completion rule,
and the way a conversation is flattened into one prompt) follows 9router's `muse-web` executor
(`open-sse/executors/muse-web/dom.js`, `prompt.js` and `muse-web.js`, read on 2026-11-12).
https://github.com/decolua/9router

MIT License

Copyright (c) 2024-2026 decolua and contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## OpenAI Codex

Codex's model catalog (`apps/desktop/src-tauri/src/codex_catalog.rs`) carries Codex's default instructions for a
model it does not know, unchanged: `codex-rs/models-manager/prompt.md` of Codex CLI 0.163.0 (tag `rust-v0.163.0`),
kept in `apps/desktop/src-tauri/third_party/codex/prompt.md` with Codex's `LICENSE` (Apache License 2.0) and
`NOTICE` beside it.
https://github.com/openai/codex

OpenAI Codex
Copyright 2025 OpenAI

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance
with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0, or in
`apps/desktop/src-tauri/third_party/codex/LICENSE`. Unless required by applicable law or agreed to in writing,
software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF
ANY KIND, either express or implied. See the License for the specific language governing permissions and
limitations under the License.
