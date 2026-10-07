// The shared-memory region between Wi_WWAV.app and wwav-engine (docs/ENGINE.md §4).
//
// The app creates and zeroes the region; the engine maps it, writes the
// header, then writes the clock, crumb and meters from the audio thread
// without locks. crates/wwav-wire/src/shm.rs mirrors this layout, and a test
// compiles this header and compares every offsetof with the Rust side.
//
// Little-endian, every field naturally aligned. C99 and C++17 both read it.

#ifndef WWAV_SHM_H
#define WWAV_SHM_H

#include <stddef.h>
#include <stdint.h>

#define WWAV_SHM_MAGIC "WWAV"
#define WWAV_SHM_LAYOUT 1u
#define WWAV_PROTOCOL 1u

#define WWAV_METER_SLOTS 260u   /* tracks, then 4 stem buses, then master */
#define WWAV_METER_RING 64u     /* entries */
#define WWAV_METER_ENTRY_HEADER 32u
#define WWAV_METER_SLOT_BYTES 16u
#define WWAV_METER_ENTRY_BYTES (WWAV_METER_ENTRY_HEADER + WWAV_METER_SLOTS * WWAV_METER_SLOT_BYTES)

#define WWAV_STATE_STOPPED 0u
#define WWAV_STATE_PLAYING 1u
#define WWAV_STATE_RECORDING 2u

/* Offset 0, 64 bytes. */
typedef struct wwav_shm_header {
    char magic[4];             /* "WWAV" */
    uint32_t layout;           /* WWAV_SHM_LAYOUT */
    uint32_t sample_rate;
    uint32_t block_size;
    uint32_t meter_slots;      /* WWAV_METER_SLOTS */
    uint32_t meter_ring;       /* WWAV_METER_RING */
    uint32_t peaks_bytes;
    uint32_t input_bytes;
    uint64_t engine_pid;
    uint64_t engine_start_ns;  /* monotonic */
    uint8_t reserved[16];
} wwav_shm_header;

/* Offset 64, 64 bytes: a seqlock. seq is odd while the fields are written. */
typedef struct wwav_shm_clock {
    uint64_t seq;
    int64_t sample_pos;        /* where the playhead will be when this block reaches the speaker */
    uint64_t host_time_ns;     /* monotonic time at which sample_pos is at the speaker */
    double rate;               /* samples per second of playhead travel; 0 when stopped */
    uint32_t state;            /* WWAV_STATE_* */
    uint32_t dropouts;
    uint64_t callbacks;
    uint8_t reserved[16];
} wwav_shm_clock;

/* Offset 128, 64 bytes. */
typedef struct wwav_shm_crumb {
    uint64_t crumb;            /* FNV-1a 64 of the graph node id now processing; 0 when none */
    uint64_t crumb_seq;
    uint8_t reserved[48];
} wwav_shm_crumb;

/* Offset 192, 64 bytes: the meter ring's write counter. */
typedef struct wwav_shm_meter_index {
    uint64_t meter_write;      /* entries written so far; entry i is at ring slot i % meter_ring */
    uint8_t reserved[56];
} wwav_shm_meter_index;

/* One slot of a meter entry: linear values. */
typedef struct wwav_shm_meter_slot {
    float peak_l, peak_r, rms_l, rms_r;
} wwav_shm_meter_slot;

/* One meter entry, at 256 + (i % meter_ring) * WWAV_METER_ENTRY_BYTES. */
typedef struct wwav_shm_meter_entry {
    uint64_t callback;
    float dsp_load;            /* 0..1 */
    uint32_t dropouts;
    uint32_t slots_used;
    uint8_t reserved[12];
    wwav_shm_meter_slot slots[WWAV_METER_SLOTS];
} wwav_shm_meter_entry;

typedef struct wwav_shm {
    wwav_shm_header header;          /* 0 */
    wwav_shm_clock clock;            /* 64 */
    wwav_shm_crumb crumb;            /* 128 */
    wwav_shm_meter_index meters;     /* 192 */
    wwav_shm_meter_entry ring[WWAV_METER_RING]; /* 256 */
    /* then the peaks ring (header.peaks_bytes) and the input ring (header.input_bytes) */
} wwav_shm;

#define WWAV_SHM_RING_OFFSET 256u
#define WWAV_SHM_PEAKS_OFFSET (WWAV_SHM_RING_OFFSET + WWAV_METER_RING * WWAV_METER_ENTRY_BYTES)
#define WWAV_SHM_FIXED_BYTES WWAV_SHM_PEAKS_OFFSET

/* The input ring holds 2 s of interleaved f32 stereo at up to 96 kHz; the
   peaks ring 64 KiB. Total = fixed part + both. */
#define WWAV_SHM_PEAKS_BYTES (64u * 1024u)
#define WWAV_SHM_INPUT_BYTES (2u * 96000u * 2u * 4u)
#define WWAV_SHM_TOTAL_BYTES (WWAV_SHM_FIXED_BYTES + WWAV_SHM_PEAKS_BYTES + WWAV_SHM_INPUT_BYTES)

#ifdef __cplusplus
static_assert(sizeof(wwav_shm_header) == 64, "header is 64 bytes");
static_assert(sizeof(wwav_shm_clock) == 64, "clock is 64 bytes");
static_assert(sizeof(wwav_shm_crumb) == 64, "crumb is 64 bytes");
static_assert(sizeof(wwav_shm_meter_entry) == WWAV_METER_ENTRY_BYTES, "meter entry size");
static_assert(offsetof(wwav_shm, clock) == 64, "clock at 64");
static_assert(offsetof(wwav_shm, crumb) == 128, "crumb at 128");
static_assert(offsetof(wwav_shm, meters) == 192, "meter index at 192");
static_assert(offsetof(wwav_shm, ring) == WWAV_SHM_RING_OFFSET, "ring at 256");
#endif

#endif /* WWAV_SHM_H */
