// Trigonometry that gives the same bits in every JavaScript engine.
//
// `Math.sin`, `Math.cos` and `Math.atan2` are only required to be close:
// V8, JavaScriptCore and Chakra each use their own library, and they can
// disagree in the last bit. A last bit is enough to move a hashed layout,
// so the sky computes its angles here instead. These are ports of fdlibm
// (Sun's freely distributable libm, the code behind most engines' own
// versions), built only from + − × ÷ and comparisons, which IEEE 754 and
// ECMAScript define exactly. They stay within one ulp of the true value.
//
// `Math.sqrt` is correctly rounded by IEEE 754, so it is exact everywhere
// and is used as it is.

export const sqrt = Math.sqrt;

// The high and low 32 bits of a double, as fdlibm reads them.
const words = new DataView(new ArrayBuffer(8));

function highWord(x: number): number {
  words.setFloat64(0, x);
  return words.getInt32(0);
}

function lowWord(x: number): number {
  words.setFloat64(0, x);
  return words.getUint32(4);
}

function withHighWord(hi: number): number {
  words.setInt32(0, hi);
  words.setUint32(4, 0);
  return words.getFloat64(0);
}

// --- sin and cos -----------------------------------------------------------

const S1 = -1.66666666666666324348e-1;
const S2 = 8.33333333332248946124e-3;
const S3 = -1.98412698298579493134e-4;
const S4 = 2.75573137070700676789e-6;
const S5 = -2.50507602534068634195e-8;
const S6 = 1.58969099521155010221e-10;

// sin on [−π/4, π/4], where x + y is the argument and y its tail.
function kernelSin(x: number, y: number, tail: boolean): number {
  if ((highWord(x) & 0x7fffffff) < 0x3e400000) return x; // |x| < 2^−27
  const z = x * x;
  const v = z * x;
  const r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
  if (!tail) return x + v * (S1 + z * r);
  return x - (z * (0.5 * y - v * r) - y - v * S1);
}

const C1 = 4.16666666666666019037e-2;
const C2 = -1.38888888888741095749e-3;
const C3 = 2.48015872894767294178e-5;
const C4 = -2.75573143513906633035e-7;
const C5 = 2.08757232129817482790e-9;
const C6 = -1.13596475577881948265e-11;

// cos on [−π/4, π/4].
function kernelCos(x: number, y: number): number {
  const ix = highWord(x) & 0x7fffffff;
  if (ix < 0x3e400000) return 1; // |x| < 2^−27
  const z = x * x;
  const r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
  if (ix < 0x3fd33333) return 1 - (0.5 * z - (z * r - x * y)); // |x| < 0.3
  // Split off a quarter of x (or 0.28125) so 1 − ½z loses no bits.
  const qx = ix > 0x3fe90000 ? 0.28125 : withHighWord(ix - 0x00200000);
  const hz = 0.5 * z - qx;
  return 1 - qx - (hz - (z * r - x * y));
}

const INV_PIO2 = 6.36619772367581382433e-1;
// π/2 in three pieces of 33 bits plus tails, so n · piece is exact.
const PIO2_1 = 1.57079632673412561417;
const PIO2_1T = 6.07710050650619224932e-11;
const PIO2_2 = 6.07710050630396597660e-11;
const PIO2_2T = 2.02226624879595063154e-21;
const PIO2_3 = 2.02226624871116645580e-21;
const PIO2_3T = 8.47842766036889956997e-32;
const TWO_PI = 2 * Math.PI;

// The reduced argument, head and tail, written by reduce().
let reducedHead = 0;
let reducedTail = 0;

// x − n·π/2 for the nearest n, in reducedHead + reducedTail; returns n.
// Exact to well under an ulp for |x| ≤ 2^19·π/2 (about 823,000). Beyond
// that, x is first taken modulo the double nearest 2π, which `%` does
// exactly: still the same bits everywhere, but the angle drifts by about
// 4e−17 per radian. Nothing in the sky turns that far.
function reduce(x: number): number {
  let hx = highWord(x);
  let ix = hx & 0x7fffffff;
  if (ix > 0x413921fb) {
    x %= TWO_PI;
    hx = highWord(x);
    ix = hx & 0x7fffffff;
  }
  const t = Math.abs(x);
  const n = Math.floor(t * INV_PIO2 + 0.5);
  let r = t - n * PIO2_1;
  let w = n * PIO2_1T;
  let y0 = r - w;
  // When x is close to a multiple of π/2, the first subtraction cancels
  // most of the bits; a second (and third) piece of π/2 restores them.
  const j = ix >> 20;
  if (j - ((highWord(y0) >> 20) & 0x7ff) > 16) {
    let s = r;
    w = n * PIO2_2;
    r = s - w;
    w = n * PIO2_2T - (s - r - w);
    y0 = r - w;
    if (j - ((highWord(y0) >> 20) & 0x7ff) > 49) {
      s = r;
      w = n * PIO2_3;
      r = s - w;
      w = n * PIO2_3T - (s - r - w);
      y0 = r - w;
    }
  }
  const y1 = r - y0 - w;
  if (hx < 0) {
    reducedHead = -y0;
    reducedTail = -y1;
    return -n;
  }
  reducedHead = y0;
  reducedTail = y1;
  return n;
}

export function sin(x: number): number {
  const ix = highWord(x) & 0x7fffffff;
  if (ix <= 0x3fe921fb) return kernelSin(x, 0, false); // |x| ≤ π/4
  if (ix >= 0x7ff00000) return NaN;
  const n = reduce(x);
  switch (n & 3) {
    case 0:
      return kernelSin(reducedHead, reducedTail, true);
    case 1:
      return kernelCos(reducedHead, reducedTail);
    case 2:
      return -kernelSin(reducedHead, reducedTail, true);
    default:
      return -kernelCos(reducedHead, reducedTail);
  }
}

export function cos(x: number): number {
  const ix = highWord(x) & 0x7fffffff;
  if (ix <= 0x3fe921fb) return kernelCos(x, 0);
  if (ix >= 0x7ff00000) return NaN;
  const n = reduce(x);
  switch (n & 3) {
    case 0:
      return kernelCos(reducedHead, reducedTail);
    case 1:
      return -kernelSin(reducedHead, reducedTail, true);
    case 2:
      return -kernelCos(reducedHead, reducedTail);
    default:
      return kernelSin(reducedHead, reducedTail, true);
  }
}

// --- atan and atan2 -------------------------------------------------------

const ATAN_HI = [
  4.63647609000806093515e-1, 7.85398163397448278999e-1, 9.82793723247329054082e-1,
  1.57079632679489655800,
];
const ATAN_LO = [
  2.26987774529616870924e-17, 3.06161699786838301793e-17, 1.39033110312309984516e-17,
  6.12323399573676603587e-17,
];
const AT = [
  3.33333333333329318027e-1, -1.99999999998764832476e-1, 1.42857142725034663711e-1,
  -1.11111104054623557880e-1, 9.09088713343650656196e-2, -7.69187620504482999495e-2,
  6.66107313738753120669e-2, -5.83357013379057348645e-2, 4.97687799461593236017e-2,
  -3.65315727442169155270e-2, 1.62858201153657823623e-2,
];

function atan(x: number): number {
  const hx = highWord(x);
  const ix = hx & 0x7fffffff;
  if (ix >= 0x44100000) {
    // |x| ≥ 2^66
    if (Number.isNaN(x)) return NaN;
    return hx > 0 ? ATAN_HI[3] + ATAN_LO[3] : -ATAN_HI[3] - ATAN_LO[3];
  }
  // Reduce |x| onto a small interval around one of four known arctangents.
  let id: number;
  if (ix < 0x3fdc0000) {
    // |x| < 7/16
    if (ix < 0x3e400000) return x; // |x| < 2^−27
    id = -1;
  } else {
    x = Math.abs(x);
    if (ix < 0x3ff30000) {
      if (ix < 0x3fe60000) {
        id = 0; // 7/16 ≤ |x| < 11/16
        x = (2 * x - 1) / (2 + x);
      } else {
        id = 1; // 11/16 ≤ |x| < 19/16
        x = (x - 1) / (x + 1);
      }
    } else if (ix < 0x40038000) {
      id = 2; // 19/16 ≤ |x| < 39/16
      x = (x - 1.5) / (1 + 1.5 * x);
    } else {
      id = 3; // 39/16 ≤ |x| < 2^66
      x = -1 / x;
    }
  }
  const z = x * x;
  const w = z * z;
  const s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
  const s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
  if (id < 0) return x - x * (s1 + s2);
  const result = ATAN_HI[id] - (x * (s1 + s2) - ATAN_LO[id] - x);
  return hx < 0 ? -result : result;
}

const PI = Math.PI;
const PI_LO = 1.2246467991473531772e-16;
const PI_O_2 = Math.PI / 2;
const PI_O_4 = Math.PI / 4;

export function atan2(y: number, x: number): number {
  if (Number.isNaN(x) || Number.isNaN(y)) return NaN;
  if (x === 1) return atan(y);
  const hx = highWord(x);
  const hy = highWord(y);
  const ix = hx & 0x7fffffff;
  const iy = hy & 0x7fffffff;
  // 2 · sign(x) + sign(y), reading the sign bits so −0 counts as negative.
  const m = (hy < 0 ? 1 : 0) | (hx < 0 ? 2 : 0);

  if (y === 0) {
    if (m < 2) return y; // atan2(±0, +x) = ±0
    return m === 2 ? PI : -PI;
  }
  if (x === 0) return hy < 0 ? -PI_O_2 : PI_O_2;
  if (ix === 0x7ff00000 && lowWord(x) === 0) {
    if (iy === 0x7ff00000 && lowWord(y) === 0) {
      return [PI_O_4, -PI_O_4, 3 * PI_O_4, -3 * PI_O_4][m];
    }
    return [0, -0, PI, -PI][m];
  }
  if (iy === 0x7ff00000 && lowWord(y) === 0) return hy < 0 ? -PI_O_2 : PI_O_2;

  const k = (iy - ix) >> 20;
  let z: number;
  if (k > 60) z = PI_O_2 + 0.5 * PI_LO; // |y/x| > 2^60
  else if (hx < 0 && k < -60) z = 0; // |y|/x < −2^60
  else z = atan(Math.abs(y / x));
  switch (m) {
    case 0:
      return z;
    case 1:
      return -z;
    case 2:
      return PI - (z - PI_LO);
    default:
      return z - PI_LO - PI;
  }
}
