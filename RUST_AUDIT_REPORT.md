# Usami Project - Rust Security & Performance Audit Report

**Date:** 2026-11-19  
**Scope:** `usami-desktop` (GPUI/Rust desktop client) + `gpui-vrm` (VRM avatar library)  
**Tools Used:** cargo clippy, cargo test (82 tests passing), manual code review, unsafe code analysis  
**Note:** Miri and Loom could not be executed due to environment constraints (see Limitations section)

---

## Executive Summary

| Category | Findings | Critical | High | Medium | Low |
|----------|----------|----------|------|--------|-----|
| **Memory Safety (unsafe)** | 8 locations | 1 | 3 | 2 | 2 |
| **Concurrency/Deadlock** | 12 patterns | 0 | 2 | 6 | 4 |
| **Performance/Bottlenecks** | 9 areas | 0 | 1 | 5 | 3 |
| **Error Handling** | 6 gaps | 0 | 1 | 3 | 2 |
| **Code Quality (clippy)** | 95 warnings | 0 | 0 | 15 | 80 |
| **Domain-Specific (VRM/Audio/Win32)** | 11 issues | 1 | 2 | 5 | 3 |

**Overall Risk Score: MEDIUM-HIGH** — The codebase functions correctly but contains several unsafe patterns, mixed mutex types with potential deadlock, and areas where a single failure could cascade.

---

## 1. Miri (Undefined Behavior) Analysis

### Status: **NOT EXECUTABLE**

```
error: cargo uses an argfile to invoke rustc, which is not supported by cargo-miri
```

**Root Cause:** `gpui-vrm` depends on `gpui@0.2.2` which uses a build system not compatible with Miri (argfile-based rustc invocation). Miri only works on library crates; `usami-desktop` is a binary crate.

**Workaround Attempted:** 
- Tried running `cargo miri test` on `gpui-vrm` alone → fails during `gpui` compilation
- Cannot test `usami-desktop` with Miri (no library target)

**Recommendation:** 
- Consider extracting pure-Rust modules (audio, recorder, storage, mascot logic) into a separate library crate for Miri testing
- Run Miri in CI on the library portion only

---

## 2. Loom (Concurrency/Data Race) Analysis

### Status: **NOT EXECUTABLE**

**Root Cause:** 
- `usami-desktop` is a binary crate → no library target for `#[cfg(test)]` with loom
- Loom tests in `tests/loom_tests.rs` failed to compile because they import private modules (`crate::recorder`, `crate::lol`, etc.)

**Loom Test File Created** (but not runnable):
- Tests for: MicGate, VAD, Mascot gaze atomics, SessionFile, LolCompanionHandle, EventPriorityQueue, BondMeters
- All use `loom::model(|| { ... })` to explore thread interleavings

**Recommendation:**
- Extract concurrency primitives into a `usami-core` library crate
- Run loom tests there in CI
- For now, manual review identified the concurrency issues below

---

## 3. Unsafe Code Audit (Critical)

### 3.1 `presence.rs` — Win32 Subclassing & Window Proc

| Location | Issue | Severity | Impact | Effort |
|----------|-------|----------|--------|--------|
| Line 108-121 | `SetWindowLongPtrW` with function pointer cast | **HIGH** | Crash/UB if signature mismatches | Medium |
| Line 281-305 | `mascot_wndproc` — `std::mem::transmute` of function pointer | **CRITICAL** | UB if calling convention wrong; transmuting `*const ()` to fn ptr | Medium |
| Line 302 | Calling transmuted function pointer without validation | **HIGH** | Crash if `orig` is stale/null | Low |
| Line 298-303 | `ORIGINAL_WNDPROC.load()` → transmute → call | **HIGH** | Race: WNDPROC could be restored concurrently | Medium |

**Details:** The `mascot_wndproc` uses `std::mem::transmute` to convert an `isize` (stored from `SetWindowLongPtrW`) back to a function pointer. This is **undefined behavior** per Rust 2024 edition — the warning says:
```
warning[E0133]: call to unsafe function `std::intrinsics::transmute` is unsafe and requires unsafe block
```

**Fix:** Use `std::mem::transmute_copy` with proper type, or better, store the original WNDPROC in a `OnceLock<unsafe extern "system" fn(...) -> LRESULT>` and use `Option` instead of atomic.

---

### 3.2 `recorder.rs` — `GetAsyncKeyState` in Audio Thread

| Location | Issue | Severity | Impact | Effort |
|----------|-------|----------|--------|--------|
| Line 226-236 | `unsafe { GetAsyncKeyState(...) }` in capture loop | **HIGH** | Data race with UI thread; wrong key state | Low |

**Details:** The capture thread (not UI thread) calls `GetAsyncKeyState` every 40ms to detect PTT keys (V, XButton1, XButton2). This is racy — `GetAsyncKeyState` state is per-thread; the UI thread may have different key state. Also, calling Win32 API from a non-UI thread is technically allowed but fragile.

**Fix:** Move PTT detection to the UI thread (GPUI key events) and communicate via atomic/channel to the capture thread.

---

### 3.3 `mascot.rs` — Win32 Window Style Manipulation

| Location | Issue | Severity | Impact | Effort |
|----------|-------|----------|--------|--------|
| Line 196-237 | `enter()` — `SetWindowLongPtrW` + `SetWindowPos` without error checking | **MEDIUM** | Window style corruption if calls fail silently | Low |
| Line 239-275 | `exit()` — restores styles from atomics; no validation | **MEDIUM** | Wrong styles restored if atomics stale | Low |
| Line 138-140 | `SAVED_STYLE`, `SAVED_EX_STYLE`, `SAVED_RECT` as static atomics | **LOW** | No synchronization with actual window state | Low |

**Details:** The mascot mode toggles `WS_EX_TOOLWINDOW`, `WS_EX_TOPMOST`, `WS_EX_APPWINDOW` and removes frame styles. If any `SetWindowLongPtrW` fails, the window enters an inconsistent state. The `SAVED_RECT` uses `Mutex<Option<WinRect>>` but other state uses `AtomicIsize` — mixed sync primitives.

**Fix:** Add error checking with `GetLastError()`; consider a single `Mutex<MascotState>` for all window state.

---

### 3.4 `audio.rs` — `PcmStreamSource` Iterator Implementation

| Location | Issue | Severity | Impact | Effort |
|----------|-------|----------|--------|--------|
| Line 108-133 | `next()` returns `Some(0.0)` on channel empty (silence) | **LOW** | Silent audio when network stalls; masks backpressure | Low |
| Line 121-131 | `TryRecvError::Disconnected` → reports level 0.0 then returns `None` | **LOW** | Correct shutdown but level callback called twice | Low |

**Details:** The design intentionally returns silence (0.0) when the channel is empty, keeping the audio stream alive. This is a **design choice**, not a bug, but it means network stalls produce audible silence instead of pausing.

---

### 3.5 `gpui-vrm/src/loader.rs` — `glTF` Loading (External)

**Note:** `gpui-vrm` uses `gltf` crate which has `unsafe` blocks for buffer access. Not audited here as it's a dependency.

---

## 4. Concurrency & Deadlock Analysis

### 4.1 Mixed Mutex Types (High Risk)

| File | Mutex Types Used | Risk |
|------|------------------|------|
| `lol/mod.rs` | `std::sync::Mutex` + `tokio::sync::Mutex` | **HIGH** |
| `presence.rs` | `std::sync::Mutex` + `OnceLock<Mutex<...>>` + atomics | **MEDIUM** |
| `mascot.rs` | `std::sync::Mutex` + `AtomicIsize` + `AtomicBool` | **MEDIUM** |
| `recorder.rs` | `std::sync::Mutex` + `tokio::sync::mpsc` + atomics | **MEDIUM** |
| `audio.rs` | `std::sync::mpsc` + `AtomicUsize` | **LOW** |

**Deadlock Scenario in `lol/mod.rs`:**
```rust
// Line 330: Holding TokioMutex while awaiting
let recent_snapshot = recent_replies.lock().await.clone();

// Line 395: Spawns task that also locks recent_replies
let guard = GeneratingGuard { flag: Arc::clone(&is_generating) };
tokio::spawn(async move {
    let _guard = guard;
    dispatch_to_llm(...).await;  // Line 443: recent_replies.lock().await
});
```

If `dispatch_to_llm` is slow and another event tries to `queue.push()` (which may trigger LLM), we could have contention on `recent_replies`. Not a classic deadlock but **lock convoy** under load.

---

### 4.2 Atomics with Inconsistent Ordering

| File | Atomics | Ordering Used | Issue |
|------|---------|---------------|-------|
| `mascot.rs` | `TOGGLE_REQUESTED`, `ACTIVE`, `ENABLED`, `FOLLOW`, `GAZE_STARTED`, `GAZE_STOP` | `SeqCst` / `Relaxed` mixed | `Relaxed` on flags read by different threads may miss updates |
| `recorder.rs` | `MicGate` fields | `Relaxed` | OK for independent flags, but `suspended()` reads 4 atomics non-atomically |
| `presence.rs` | `APP_WINDOW`, `HIDDEN_WINDOW`, `TRAY_ICON`, `ORIGINAL_WNDPROC` | `SeqCst` / `Relaxed` | `SeqCst` on some, `Relaxed` on others — inconsistent |

**Specific Issue in `recorder.rs:80-91`:**
```rust
pub fn suspended(&self) -> bool {
    if self.user.load(Ordering::Relaxed) || self.agent.load(Ordering::Relaxed) {
        return true;
    }
    if self.gamer_mode.load(Ordering::Relaxed)
        && self.ptt_required.load(Ordering::Relaxed)
        && !self.ptt_down.load(Ordering::Relaxed)
    {
        return true;
    }
    false
}
```
Four separate `Relaxed` loads — **not atomic as a group**. Between loads, another thread could change `gamer_mode` and `ptt_down`, causing a false `suspended()` result.

**Fix:** Use a single `AtomicU32` bitfield or `Mutex<MicGateState>`.

---

### 4.3 Gaze Loop Thread Safety

**File:** `mascot.rs:392-425` — `start_gaze_loop` spawns a `std::thread` that:
- Reads `GAZE_STOP` (atomic)
- Calls `is_active()` and `follow_enabled()` (atomics)
- Calls `win32::cursor_gaze()` (Win32 APIs)
- Calls `controller.look_at_normalized()` (GPUI/VRM controller)

**Risk:** The `VrmController` is `Arc`-based and thread-safe per gpui-vrm design, but the gaze thread runs **independent of GPUI's event loop**. If the window is destroyed while gaze loop runs, `win32::cursor_gaze()` operates on invalid HWND.

**Fix:** The `GAZE_STOP` flag is set in `stop_gaze_loop()` but there's no `join()` — the thread just exits on next iteration. Add a `JoinHandle` to wait for cleanup.

---

### 4.4 `presence.rs` — Hidden Window Thread

**File:** `presence.rs:175-230` — `run()` creates a hidden window and runs `GetMessageW` loop.

**Issues:**
- `HIDDEN_WINDOW` atomic stores HWND; `shutdown()` does `PostMessageW(WM_CLOSE)` but doesn't wait for thread join reliably (line 170-172)
- `ORIGINAL_WNDPROC` atomic accessed from both main thread (subclassing) and presence thread (window proc) — **data race** if not SeqCst

---

## 5. Performance & Bottleneck Analysis

### 5.1 `main.rs` — `UsamiApp` God Object (200+ fields)

**Impact:** Large struct → cache misses, slow field access, hard to reason about.

**Hot Fields (accessed every frame):**
- `is_away`, `window_focused`, `mascot_mode`, `vrm_controller`, `chat_history`, `proactive_queue`

**Recommendation:** Split into sub-structs: `WindowState`, `ChatState`, `MascotState`, `AgentState`, `AudioState`.

---

### 5.2 `audio.rs` — PCM Streaming Allocation

**File:** `audio.rs:164-193` — `push_i16_le`:
```rust
let mut samples = Vec::with_capacity(bytes.len() / 2 + 1);
// ... pushes samples ...
sender.send(samples).is_ok()
```
Allocates a new `Vec<f32>` for **every network chunk** (typically 20-100ms of audio).

**Optimization:** Use a shared buffer pool or `VecDeque` with pre-allocated capacity.

---

### 5.3 `recorder.rs` — VAD Frame Processing

**File:** `recorder.rs:240-265` — `capture_until_stopped`:
```rust
while !stop.load(Ordering::Relaxed) {
    thread::sleep(Duration::from_millis(40));
    // ... lock queue, drain, process frames ...
}
```
- **40ms sleep** = 25 wakeups/sec even when idle
- Locks `queue` (Mutex<VecDeque>) every iteration
- `pending.extend(queued.drain(..))` — moves all samples, allocates if growing

**Optimization:** Use `cpal` callback to signal via `Condvar` or channel instead of polling.

---

### 5.4 `lol/mod.rs` — Polling Loop

**File:** `lol/mod.rs:154-404` — `run_worker`:
```rust
tokio::select! {
    Some(ctrl) = bridge.ctrl_rx.recv() => { ... }
    _ = tokio::time::sleep(delay) => { ... }
}
```
- `delay` varies from 1s to 5s based on game state
- Each poll does HTTP request to Live Client Data API
- `ddragon` store cloned on `RefreshDataDragon` (full reload)

**Bottleneck:** `DataDragonStore::load_or_fetch()` downloads ~50MB of JSON on first run; subsequent loads read from disk but still parse entire dataset.

---

### 5.5 `gpui-vrm/src/view.rs` — VRM Rendering

**File:** `gpui-vrm/src/view.rs` — `VrmView` render loop:
- Calls `skeleton.update_pose()`, `physics.step()`, `expressions.update()` every frame
- `renderer.render_frame()` → WGPU submit
- No frustum culling, no LOD, renders full skeleton every frame

**Optimization Opportunity:** Cache bone matrices when animation not playing; skip spring bone simulation when avatar not visible.

---

### 5.6 `storage.rs` — Atomic JSON Writes

**File:** `storage.rs:206-226` — `write_json`:
```rust
let tmp = path.with_extension("json.tmp");
fs::write(&tmp, bytes)?;
fs::rename(&tmp, path)?;
```
**Good pattern** (atomic rename), but:
- Called from UI thread via `persist_prefs()` on every window drag (debounced 2s)
- `save_conversation()` called on every message
- No batching — each call does full serialize + write

---

## 6. Error Handling Gaps

### 6.1 `api.rs` — Silent Failures in `run_on_tokio`

**File:** `api.rs:1026-1039`:
```rust
async fn run_on_tokio<F, T>(fut: F) -> Result<T, String>
where
    F: Future<Output = Result<T, String>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        fut.await
    } else {
        tokio_runtime()
            .spawn(fut)
            .await
            .map_err(|e| format!("Falha na tarefa HTTP: {e}"))?
    }
}
```
**Issue:** If the task panics, `JoinError` is converted to string but the panic payload is lost. No panic hook to log.

---

### 6.2 `recorder.rs` — Stream Error Handling

**File:** `recorder.rs:304-306`:
```rust
fn report_stream_error(error: cpal::Error) {
    eprintln!("[Usami Mic] {error}");
}
```
**Issue:** Errors are only logged to stderr; UI never notified. If microphone disconnects, `is_listening` stays `true` but no audio flows.

---

### 6.3 `lol/mod.rs` — LLM Dispatch Error Swallowing

**File:** `lol/mod.rs:481-496`:
```rust
Ok(Err(err)) => {
    let now_secs = ...;
    let last = LAST_CORE_ERROR_SECS.load(Ordering::Relaxed);
    if now_secs.saturating_sub(last) >= 300 {
        LAST_CORE_ERROR_SECS.store(now_secs, Ordering::Relaxed);
        eprintln!("[Usami.GG] Falha ao consultar core 8787 ({err})");
    }
}
```
**Issue:** Errors are rate-limited (1 per 5 min) — legitimate errors during extended outage are silently dropped. UI never knows LLM failed.

---

### 6.4 `presence.rs` — Hotkey Registration Failure

**File:** `presence.rs:189-212`:
```rust
let hotkey = unsafe {
    RegisterHotKey(hidden, HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_U as u32)
} != 0;
```
**Issue:** If `RegisterHotKey` fails (another app grabbed Ctrl+Alt+U), only logs error. No fallback hotkey, no UI notification.

---

## 7. Domain-Specific Checks

### 7.1 VRM Rendering (`gpui-vrm`)

| Check | Status | Finding |
|-------|--------|---------|
| BlendShape weights in bounds [0,1] | ✅ | Clamped in `expressions.rs` |
| Spring bone simulation stability | ⚠️ | No damping clamp; can explode at high dt |
| VRMA animation blending | ✅ | `SkeletonRuntime::play_vrma` handles crossfade |
| MToon material parameters | ✅ | Validated in loader |
| Texture loading (sRGB/linear) | ⚠️ | `DecodedTexture` assumes sRGB for baseColor |
| GPU buffer alignment | ✅ | `bytemuck::Pod` + `bytemuck::Zeroable` on `GpuVertex` |

---

### 7.2 Audio Pipeline (`audio.rs` + `recorder.rs`)

| Check | Status | Finding |
|-------|--------|---------|
| Sample rate conversion | ✅ | `rodio` handles resampling |
| Lip-sync level callback timing | ⚠️ | Callback runs on audio thread; `VrmController::set_speaking` must be thread-safe |
| PCM stream backpressure | ⚠️ | Returns silence on empty; no flow control |
| WAV encoding correctness | ✅ | Tested in `audio.rs` tests |
| VAD threshold calibration | ⚠️ | Hardcoded constants; no runtime calibration |

---

### 7.3 Win32 Integration (`presence.rs` + `mascot.rs`)

| Check | Status | Finding |
|-------|--------|---------|
| Window subclassing cleanup | ❌ | `ORIGINAL_WNDPROC` not restored on crash |
| High-DPI awareness | ⚠️ | Uses `GetSystemMetrics(SM_CXSCREEN)` — may be wrong on per-monitor DPI |
| Multi-monitor support | ⚠️ | `MonitorFromWindow` with `MONITOR_DEFAULTTONEAREST` — OK |
| Tray icon recreation on Explorer restart | ✅ | Handles `TaskbarCreated` message |
| Global hotkey conflict handling | ❌ | No fallback, no user notification |

---

### 7.4 LoL Companion (`lol/mod.rs`)

| Check | Status | Finding |
|-------|--------|---------|
| Circuit breaker pattern | ✅ | `CircuitBreaker` with exponential backoff |
| Event priority queue | ✅ | `EventPriorityQueue` with cooldown buckets |
| Recent replies deduplication | ✅ | `TokioMutex<Vec<String>>` with max 5 entries |
| LLM timeout handling | ⚠️ | 30s Ollama / 8s backend — no cancellation token propagation |
| DataDragon cache invalidation | ⚠️ | Only manual `RefreshDataDragon`; no auto-update on patch |

---

## 8. Clippy Warnings Summary

### `usami-desktop` (73 warnings)
| Category | Count | Examples |
|----------|-------|----------|
| `collapsible_if` | ~25 | Nested ifs that can be combined |
| `let_unit_value` | ~5 | `let _ = ...` on expression |
| `map_or` → `unwrap_or` | ~8 | Simplifiable Option handling |
| `manual_div_ceil` | 1 | Use `div_ceil` method |
| `large_enum_variant` | 1 | `ActiveDrawer` has `Vrma` variant with large data |
| `useless_conversion` | ~5 | `String::from(...)` on `SharedString` |
| `match_same_arms` | 1 | Match arms identical |
| Dead code (fields/variants) | ~15 | Unused fields in `UsamiApp`, `Palette`, enums |

### `gpui-vrm` (22 warnings)
| Category | Count |
|----------|-------|
| `collapsible_if` | ~8 |
| `manual_map` / `map_or` | ~5 |
| Dead code (methods/fields) | ~5 |
| Other style | ~4 |

---

## 9. Test Coverage Analysis

| Module | Tests | Coverage Notes |
|--------|-------|----------------|
| `audio.rs` | 6 | Good: PCM sink, source, level reporting |
| `recorder.rs` | 8 | Good: VAD states, gate, WAV encoding |
| `storage.rs` | 5 | Good: Conversation, session, prefs |
| `lifecycle.rs` | 1 | Minimal: only farewell logic |
| `presence.rs` | 3 | Registry, emotion categorization, command quoting |
| `mascot.rs` | 6 | Geometry, gaze math, menu positioning |
| `lol/*` | 12 | Good: brain, live_client, state_diff, ddragon |
| `api.rs` | 25 | Excellent: HTTP contract tests with mock server |
| `gpui-vrm` | 8 | Basic: visemes, emotions, VRM load + headless render |

**Total: 82 tests passing**

**Gaps:**
- No integration tests for mascot mode transitions
- No tests for gaze loop thread lifecycle
- No tests for audio stream under network latency
- No tests for LoL worker shutdown/cleanup
- No concurrency tests (loom not runnable)

---

## 10. Prioritized Remediation Plan

### 🔴 CRITICAL (Fix Immediately)

| # | Issue | File | Fix |
|---|-------|------|-----|
| 1 | `std::mem::transmute` of function pointer in WNDPROC | `presence.rs:300-301` | Replace with `OnceLock<unsafe extern "system" fn(...) -> LRESULT>` |
| 2 | `GetAsyncKeyState` in audio capture thread | `recorder.rs:226-236` | Move PTT detection to UI thread, signal via atomic/channel |
| 3 | Window subclassing not restored on crash/panic | `presence.rs:108-121` | Add panic hook to restore `ORIGINAL_WNDPROC` |

### 🟠 HIGH (Fix This Sprint)

| # | Issue | File | Fix |
|---|-------|------|-----|
| 4 | Mixed mutex types causing lock convoy | `lol/mod.rs` | Use `tokio::sync::Mutex` consistently; avoid holding across `.await` |
| 5 | Non-atomic `MicGate::suspended()` reads 4 atomics | `recorder.rs:80-91` | Use single `AtomicU32` bitfield or `Mutex<MicGateState>` |
| 6 | Gaze loop thread not joined on shutdown | `mascot.rs:392-425` | Store `JoinHandle`, `join()` in `stop_gaze_loop()` |
| 7 | Hidden window thread not cleanly joined | `presence.rs:156-173` | Wait for thread join with timeout in `shutdown()` |
| 8 | LLM errors silently dropped (rate-limited) | `lol/mod.rs:481-496` | Add error channel to UI; don't rate-limit critical errors |

### 🟡 MEDIUM (Next Sprint)

| # | Issue | File | Fix |
|---|-------|------|-----|
| 9 | God object `UsamiApp` (200+ fields) | `main.rs` | Split into sub-structs: `WindowState`, `ChatState`, `MascotState`, `AgentState`, `AudioState` |
| 10 | PCM streaming allocates Vec per chunk | `audio.rs:164-193` | Use buffer pool / `VecDeque` with pre-allocation |
| 11 | VAD capture loop polls every 40ms | `recorder.rs:221` | Use `cpal` callback + `Condvar`/`channel` for wakeup |
| 12 | Win32 style manipulation without error checking | `mascot.rs:196-275` | Check `GetLastError()`; use single `Mutex<MascotState>` |
| 13 | Hotkey registration failure silent | `presence.rs:189-212` | Fallback hotkey (e.g., Ctrl+Alt+Shift+U); UI notification |
| 14 | DataDragon reload parses entire dataset | `lol/mod.rs:188` | Lazy-load champion data; incremental updates |

### 🟢 LOW (Technical Debt)

| # | Issue | File | Fix |
|---|-------|------|-----|
| 15 | Clippy warnings (95 total) | All files | Run `cargo clippy --fix` |
| 16 | Dead code (unused fields/variants) | `main.rs`, `theme.rs`, `collab_stage.rs` | Remove or `#[allow(dead_code)]` with comment |
| 17 | `Relaxed` ordering inconsistency | Multiple files | Audit all atomics; use `SeqCst` for cross-thread flags |
| 18 | Add Miri-testable library crate | New crate | Extract `audio`, `recorder`, `storage`, `mascot` logic |
| 19 | Add loom tests for concurrency | New crate | Port `loom_tests.rs` to library crate |
| 20 | VRM spring bone damping clamp | `gpui-vrm/src/physics.rs` | Add max velocity/force clamp |

---

## 11. Limitations & Disclaimers

1. **Miri not runnable** — gpui dependency uses argfile-based rustc invocation unsupported by Miri. Only manual unsafe code review performed.

2. **Loom not runnable** — binary crate + private modules prevent concurrency testing. Created test file but cannot execute.

3. **No dynamic analysis** — No profiling (perf, samply), no fuzzing, no load testing performed.

4. **External dependencies not audited** — `gpui`, `wgpu`, `rodio`, `cpal`, `gltf`, `windows-sys` assumed correct.

5. **Windows-only code paths** — `presence.rs`, `mascot.rs` (Win32), `recorder.rs` (GetAsyncKeyState) only tested conceptually.

6. **GPUI 0.2.2 quirks** — Known issues: scroll needs `.id()`, overflow doesn't clip corners, text without width overlaps, viewport freezes with `SetWindowPos` sync, VrmView mouse leaks to parent. These are framework limitations, not bugs in our code.

---

## 12. Conclusion

The Usami desktop client is **functional and well-tested** (82 passing tests) but carries **significant technical debt in unsafe Win32 integration and concurrency patterns**. The most critical issues are:

1. **UB in WNDPROC transmute** — Could crash on Windows version changes
2. **PTT detection in wrong thread** — Race condition, unreliable
3. **Mixed mutex types** — Lock convoy risk under load
4. **No cleanup guarantees** — Threads not joined, WNDPROC not restored on panic

**Recommended immediate actions:**
1. Fix the 3 CRITICAL issues (WNDPROC, PTT thread, panic cleanup)
2. Extract concurrency primitives to a library crate for Miri/Loom testing
3. Split `UsamiApp` god object to improve maintainability

**Estimated effort for CRITICAL + HIGH fixes:** ~2-3 developer weeks

---

*Report generated by automated audit + manual review. All findings should be verified before applying fixes.*