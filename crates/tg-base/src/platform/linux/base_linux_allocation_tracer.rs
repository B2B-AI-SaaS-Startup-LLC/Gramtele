//! Enabled allocation tracer is partial stage1/allocator-tracer-hooks.
//! Original SetMallocLogger/SetReallocLogger/etc require process-wide allocator
//! interposition and have no definitions in the pinned clone. No fake hook
//! installation or no-op enabled tracer is exposed here. Native global allocator
//! replacement is outside the authorized test scope; see platform-linux-2-map.md.
