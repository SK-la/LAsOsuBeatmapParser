//! Ez2Lazer MinaCalc integration using its vendored MIT 0.74.0 WebAssembly engine.
//!
//! The binary and provenance are in `resources/EzSkills/`; its cap-lift patch is part of the contract. The
//! playable Mania converter supplies notes and key count, this module performs Mina's row conversion, then calls
//! the same eight-axis f32 engine that Ez2Lazer uses. The mutable Wasm store/instance is serialized per engine.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::mania::ManiaNote;
use crate::skills::{EzMinaSkillAxis, SkillVector};

pub const ENGINE_VERSION: &str = "0.74.0";
pub const MIN_KEY_COUNT: usize = 4;
pub const MAX_KEY_COUNT: usize = 18;
pub const MIN_RATEABLE_ROWS: usize = 2;
pub const CHART_GOAL: f32 = 0.93;
pub const MIN_PLAYER_GOAL: f32 = 0.8;
pub const MAX_PLAYER_GOAL: f32 = 0.9975;
pub const RAW_AXIS_COUNT: usize = 8;

/// One MinaCalc row: a bit per active column and its time in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinaRow {
    pub column_mask: u32,
    pub time_seconds: f32,
}

/// Converts playable Mania notes to MinaCalc rows, joining same-rounded-ms heads into a chord.
///
/// Holds contribute their heads only. Negative charts are shifted so the earliest row is zero, preserving all
/// inter-row intervals. Columns outside 0..32 are skipped like the Ez2Lazer converter.
pub fn convert_rows(notes: &[ManiaNote]) -> Vec<MinaRow> {
    if notes.is_empty() {
        return Vec::new();
    }

    let sorted = notes
        .windows(2)
        .all(|pair| pair[0].start_time_ms <= pair[1].start_time_ms);
    let mut rows = Vec::<(i64, u32)>::new();

    if sorted {
        for note in notes {
            if note.column >= 32 || !note.start_time_ms.is_finite() {
                continue;
            }
            let time = note.start_time_ms.round_ties_even() as i64;
            if let Some((last_time, mask)) = rows.last_mut() {
                if *last_time == time {
                    *mask |= 1u32 << note.column;
                    continue;
                }
            }
            rows.push((time, 1u32 << note.column));
        }
    } else {
        let mut by_time = BTreeMap::<i64, u32>::new();
        for note in notes {
            if note.column >= 32 || !note.start_time_ms.is_finite() {
                continue;
            }
            let time = note.start_time_ms.round_ties_even() as i64;
            *by_time.entry(time).or_default() |= 1u32 << note.column;
        }
        rows.extend(by_time);
    }

    let offset = rows.first().map_or(0_i64, |(time, _)| (-*time).max(0));
    rows.into_iter()
        .map(|(time, column_mask)| MinaRow {
            column_mask,
            time_seconds: ((time + offset) as f32) / 1000.0,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MinaError {
    UnsupportedKeyCount(usize),
    TooFewRows(usize),
    InvalidRate,
    InvalidGoal,
    ChordTooWide { row: usize, count: u32 },
    Runtime(String),
}

impl Display for MinaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedKeyCount(keys) => write!(
                f,
                "MinaCalc supports {MIN_KEY_COUNT}..={MAX_KEY_COUNT} keys, got {keys}"
            ),
            Self::TooFewRows(rows) => write!(
                f,
                "MinaCalc requires at least {MIN_RATEABLE_ROWS} rows, got {rows}"
            ),
            Self::InvalidRate => f.write_str("MinaCalc rate must be finite and positive"),
            Self::InvalidGoal => f.write_str("MinaCalc goal must be finite"),
            Self::ChordTooWide { row, count } => write!(
                f,
                "MinaCalc cannot process {count} simultaneous keys in row {row}"
            ),
            Self::Runtime(message) => write!(f, "MinaCalc WASM failed: {message}"),
        }
    }
}

impl Error for MinaError {}

/// Output order matches `EzMinaSkillAxisData.RAW_OUTPUT_ORDER` and the upstream module ABI.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MinaRawOutput {
    pub axes: [f32; RAW_AXIS_COUNT],
}

/// Chart and player skill axes backed by the same MinaCalc module as Ez2Lazer.
///
/// Each instance owns a mutable Wasm store and is safe for concurrent callers through an internal mutex. The
/// compiled module is cached process-wide; callers with parallel chart jobs should use a small engine pool.
pub struct MinaEngine {
    instance: std::sync::Mutex<MinaWasmInstance>,
}

impl MinaEngine {
    pub fn new() -> Result<Self, MinaError> {
        Ok(Self {
            instance: std::sync::Mutex::new(create_instance()?),
        })
    }

    pub fn calculate_chart(
        &self,
        notes: &[ManiaNote],
        key_count: usize,
        rate: f32,
    ) -> Result<SkillVector<EzMinaSkillAxis>, MinaError> {
        self.calculate(notes, key_count, rate, CHART_GOAL)
    }

    pub fn calculate_player(
        &self,
        notes: &[ManiaNote],
        key_count: usize,
        rate: f32,
        goal: f32,
    ) -> Result<SkillVector<EzMinaSkillAxis>, MinaError> {
        if !goal.is_finite() {
            return Err(MinaError::InvalidGoal);
        }
        self.calculate(
            notes,
            key_count,
            rate,
            goal.clamp(MIN_PLAYER_GOAL, MAX_PLAYER_GOAL),
        )
    }

    fn calculate(
        &self,
        notes: &[ManiaNote],
        key_count: usize,
        mut rate: f32,
        goal: f32,
    ) -> Result<SkillVector<EzMinaSkillAxis>, MinaError> {
        if !(MIN_KEY_COUNT..=MAX_KEY_COUNT).contains(&key_count) {
            return Ok(SkillVector::zeros());
        }
        if !rate.is_finite() || rate <= 0.0 {
            rate = 1.0;
        }
        let rows = convert_rows(notes);
        if rows.len() < MIN_RATEABLE_ROWS {
            return Ok(SkillVector::zeros());
        }
        for (index, row) in rows.iter().enumerate() {
            let count = row.column_mask.count_ones();
            if count > 16 {
                return Err(MinaError::ChordTooWide { row: index, count });
            }
        }
        let output = self.compute_rows(&rows, key_count, rate, goal)?;
        SkillVector::from_values(output.axes.map(f64::from).to_vec())
            .map_err(|error| MinaError::Runtime(error.to_string()))
    }

    fn compute_rows(
        &self,
        rows: &[MinaRow],
        key_count: usize,
        rate: f32,
        goal: f32,
    ) -> Result<MinaRawOutput, MinaError> {
        let mut instance = self
            .instance
            .lock()
            .map_err(|_| MinaError::Runtime("MinaCalc instance mutex was poisoned".to_owned()))?;
        match instance.compute(rows, key_count, rate, goal) {
            Ok(output) if is_floor_output(&output.axes) => {
                let mut fresh = create_instance()?;
                let retry = fresh.compute(rows, key_count, rate, goal);
                *instance = fresh;
                retry
            }
            Ok(output) => Ok(output),
            Err(error) => {
                *instance = create_instance()?;
                Err(error)
            }
        }
    }
}

struct MinaWasmInstance {
    store: wasmi::Store<()>,
    memory: wasmi::Memory,
    malloc: wasmi::TypedFunc<i32, i32>,
    free: wasmi::TypedFunc<i32, ()>,
    compute: wasmi::TypedFunc<(i32, f32, f32, i32, i32, i32, i32), i32>,
}

struct CompiledMinaModule {
    engine: wasmi::Engine,
    module: wasmi::Module,
}

fn create_instance() -> Result<MinaWasmInstance, MinaError> {
    use wasmi::{Caller, Engine, Extern, Linker, Module, Store};

    static COMPILED: std::sync::OnceLock<Result<CompiledMinaModule, String>> =
        std::sync::OnceLock::new();
    let compiled = COMPILED.get_or_init(|| {
        let engine = Engine::default();
        let wasm = include_bytes!("../resources/EzSkills/minaclac-74.0.wasm");
        let module = Module::new(&engine, &mut &wasm[..]).map_err(|error| error.to_string())?;
        Ok(CompiledMinaModule { engine, module })
    });
    let compiled = compiled
        .as_ref()
        .map_err(|error| MinaError::Runtime(format!("module compilation failed: {error}")))?;
    let engine = &compiled.engine;
    let module = &compiled.module;
    let mut linker = Linker::<()>::new(engine);

    fn zero_i32(caller: &mut Caller<'_, ()>, pointer: i32) {
        if let Some(Extern::Memory(memory)) = caller.get_export("memory") {
            let _ = memory.write(caller, pointer as usize, &0_i32.to_le_bytes());
        }
    }

    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |mut caller: Caller<'_, ()>, _fd: i32, _iov: i32, _count: i32, written: i32| {
                zero_i32(&mut caller, written);
                0_i32
            },
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_read",
            |mut caller: Caller<'_, ()>, _fd: i32, _iov: i32, _count: i32, read: i32| {
                zero_i32(&mut caller, read);
                0_i32
            },
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap("wasi_snapshot_preview1", "fd_close", |_fd: i32| 0_i32)
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_seek",
            |mut caller: Caller<'_, ()>, _fd: i32, _offset: i64, _whence: i32, result: i32| {
                if let Some(Extern::Memory(memory)) = caller.get_export("memory") {
                    let _ = memory.write(&mut caller, result as usize, &0_i64.to_le_bytes());
                }
                0_i32
            },
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "environ_sizes_get",
            |mut caller: Caller<'_, ()>, count: i32, size: i32| {
                zero_i32(&mut caller, count);
                zero_i32(&mut caller, size);
                0_i32
            },
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "environ_get",
            |_env: i32, _buffer: i32| 0_i32,
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap("env", "_abort_js", || -> Result<(), wasmi::Error> {
            Err(wasmi::Error::new("MinaCalc called abort"))
        })
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap("env", "_tzset_js", |_: i32, _: i32, _: i32, _: i32| {})
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    linker
        .func_wrap(
            "env",
            "emscripten_resize_heap",
            |mut caller: Caller<'_, ()>, requested: i32| -> i32 {
                let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
                    return 0;
                };
                let requested = requested as u32 as u64;
                let needed = (requested + 65_535) / 65_536;
                let current = memory.size(&caller);
                if needed <= current {
                    return 1;
                }
                memory.grow(&mut caller, needed - current).map_or(0, |_| 1)
            },
        )
        .map_err(|error| MinaError::Runtime(error.to_string()))?;

    let mut store = Store::new(engine, ());
    let instance = linker
        .instantiate_and_start(&mut store, module)
        .map_err(|error| MinaError::Runtime(format!("module instantiation failed: {error}")))?;
    if let Some(constructors) = instance
        .get_typed_func::<(), ()>(&mut store, "__wasm_call_ctors")
        .ok()
    {
        constructors
            .call(&mut store, ())
            .map_err(|error| MinaError::Runtime(error.to_string()))?;
    }
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| MinaError::Runtime("WASM memory export missing".to_owned()))?;
    let malloc = instance
        .get_typed_func::<i32, i32>(&mut store, "malloc")
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    let free = instance
        .get_typed_func::<i32, ()>(&mut store, "free")
        .map_err(|error| MinaError::Runtime(error.to_string()))?;
    let compute = instance
        .get_typed_func::<(i32, f32, f32, i32, i32, i32, i32), i32>(&mut store, "minacalc_compute")
        .map_err(|error| MinaError::Runtime(error.to_string()))?;

    Ok(MinaWasmInstance {
        store,
        memory,
        malloc,
        free,
        compute,
    })
}

fn is_floor_output(raw: &[f32; RAW_AXIS_COUNT]) -> bool {
    let stream = raw[1];
    stream > 0.0
        && stream == raw[7]
        && stream == raw[6]
        && stream == raw[2]
        && stream == raw[3]
        && stream == raw[5]
}

impl MinaWasmInstance {
    fn compute(
        &mut self,
        rows: &[MinaRow],
        key_count: usize,
        rate: f32,
        goal: f32,
    ) -> Result<MinaRawOutput, MinaError> {
        let count = i32::try_from(rows.len())
            .map_err(|_| MinaError::Runtime("too many Mina rows".to_owned()))?;
        let bytes = count
            .checked_mul(4)
            .ok_or_else(|| MinaError::Runtime("Mina input size overflow".to_owned()))?;
        let mask_ptr = self
            .malloc
            .call(&mut self.store, bytes)
            .map_err(|error| MinaError::Runtime(error.to_string()))?;
        let time_ptr = self
            .malloc
            .call(&mut self.store, bytes)
            .map_err(|error| MinaError::Runtime(error.to_string()))?;
        let output_ptr = self
            .malloc
            .call(&mut self.store, (RAW_AXIS_COUNT * 4) as i32)
            .map_err(|error| MinaError::Runtime(error.to_string()))?;
        if mask_ptr == 0 || time_ptr == 0 || output_ptr == 0 {
            for pointer in [mask_ptr, time_ptr, output_ptr] {
                if pointer != 0 {
                    let _ = self.free.call(&mut self.store, pointer);
                }
            }
            return Err(MinaError::Runtime(
                "MinaCalc malloc returned null".to_owned(),
            ));
        }

        let mut masks = Vec::with_capacity(rows.len() * 4);
        let mut times = Vec::with_capacity(rows.len() * 4);
        for row in rows {
            masks.extend_from_slice(&row.column_mask.to_le_bytes());
            times.extend_from_slice(&row.time_seconds.to_le_bytes());
        }
        let result = (|| {
            self.memory
                .write(&mut self.store, mask_ptr as usize, &masks)
                .map_err(|error| MinaError::Runtime(error.to_string()))?;
            self.memory
                .write(&mut self.store, time_ptr as usize, &times)
                .map_err(|error| MinaError::Runtime(error.to_string()))?;
            self.memory
                .write(
                    &mut self.store,
                    output_ptr as usize,
                    &[0; RAW_AXIS_COUNT * 4],
                )
                .map_err(|error| MinaError::Runtime(error.to_string()))?;
            let status = self
                .compute
                .call(
                    &mut self.store,
                    (
                        key_count as i32,
                        rate,
                        goal,
                        mask_ptr,
                        time_ptr,
                        count,
                        output_ptr,
                    ),
                )
                .map_err(|error| MinaError::Runtime(error.to_string()))?;
            if status == 0 {
                return Err(MinaError::Runtime(
                    "minacalc_compute returned failure".to_owned(),
                ));
            }
            let mut raw = [0_u8; RAW_AXIS_COUNT * 4];
            self.memory
                .read(&self.store, output_ptr as usize, &mut raw)
                .map_err(|error| MinaError::Runtime(error.to_string()))?;
            let mut axes = [0.0_f32; RAW_AXIS_COUNT];
            for index in 0..RAW_AXIS_COUNT {
                axes[index] = f32::from_le_bytes(raw[index * 4..index * 4 + 4].try_into().unwrap());
            }
            Ok(MinaRawOutput { axes })
        })();
        for pointer in [mask_ptr, time_ptr, output_ptr] {
            let _ = self.free.call(&mut self.store, pointer);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_round_to_milliseconds_merge_chords_and_shift_negative_time() {
        let rows = convert_rows(&[
            ManiaNote {
                start_time_ms: -20.5,
                end_time_ms: -20.5,
                column: 0,
            },
            ManiaNote {
                start_time_ms: -20.49,
                end_time_ms: 100.0,
                column: 3,
            },
            ManiaNote {
                start_time_ms: 979.5,
                end_time_ms: 979.5,
                column: 2,
            },
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].column_mask, 0b1001);
        assert_eq!(rows[0].time_seconds, 0.0);
        assert_eq!(rows[1].time_seconds, 1.0);
    }

    #[test]
    fn unsupported_keys_and_chords_are_rejected_and_short_charts_are_zero() {
        let engine = MinaEngine::new().unwrap();
        let too_few = [ManiaNote {
            start_time_ms: 0.0,
            end_time_ms: 0.0,
            column: 0,
        }];
        assert_eq!(
            engine.calculate_chart(&too_few, 4, 1.0).unwrap(),
            SkillVector::zeros()
        );
        assert_eq!(
            engine.calculate_chart(&too_few, 3, 1.0).unwrap(),
            SkillVector::zeros()
        );
        let wide_chord: Vec<_> = (0..34)
            .map(|index| ManiaNote {
                start_time_ms: if index < 17 { 0.0 } else { 100.0 },
                end_time_ms: if index < 17 { 0.0 } else { 100.0 },
                column: index % 17,
            })
            .collect();
        assert!(matches!(
            engine.calculate_chart(&wide_chord, 4, 1.0),
            Err(MinaError::ChordTooWide { .. })
        ));
    }

    #[test]
    fn stream_chart_matches_the_vendored_csharp_baseline() {
        let notes: Vec<_> = (0..64)
            .map(|index| ManiaNote {
                start_time_ms: index as f64 * 50.0,
                end_time_ms: index as f64 * 50.0,
                column: index % 4,
            })
            .collect();
        let engine = MinaEngine::new().unwrap();
        let result = engine.calculate_chart(&notes, 4, 1.0).unwrap();
        assert!((result.get(EzMinaSkillAxis::Overall) - 13.2229395).abs() < 0.25);
        assert!((result.get(EzMinaSkillAxis::Stream) - 12.41642).abs() < 0.25);
        assert!((result.get(EzMinaSkillAxis::Technical) - 12.248576).abs() < 0.25);
    }
}
