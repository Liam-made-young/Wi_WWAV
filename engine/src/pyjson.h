#pragma once
// A .wwav's JSON chunks as the reference reader sees them
// (formats/prana/tools/wwav_pack.py): json.loads(payload.decode("utf-8")),
// then the values printed with str() in its verdicts. The engine gives that
// reader's verdict on a file's stems word for word (media.cpp), so it reads
// the JSON with Python's rules and prints values as Python does.
#include <stdint.h>

#include <string>
#include <vector>

namespace wwav::py {

// One member of a JSON object, as Python's json module makes it.
struct Value {
  enum Kind { Missing, None, Bool, Int, Float, Str, List, Dict };
  Kind kind = Missing;
  bool b = false;    // Bool
  std::string text;  // Int: as written. Str: decoded, as UTF-8 (a lone surrogate in its 3-byte form)
  double f = 0.0;    // Float, NaN and the infinities among them
  std::string str;   // str() of it: a string itself, anything else its repr; "" when Missing
};

// The top-level members named `keys` of the object `payload` holds, into
// `values` (one per key; the last of a name wins, as in a dict). False when
// json.loads(payload.decode("utf-8")) would raise, or gives no dict.
bool readObject(const std::string& payload, const std::vector<std::string>& keys, std::vector<Value>* values);

// repr(), and str(), of a float: the shortest digits that read back the same.
std::string floatRepr(double d);

// re.match(r"^\d+", s): false when `s` doesn't start with a decimal digit.
// \d takes any script's (Unicode's Nd), as int() does. `positive` is
// whether int() of the digits matched is above 0.
bool leadingDigits(const std::string& s, bool* positive);

// Python's v == n, for a whole number n below 2^53.
bool equals(const Value& v, uint64_t n);

}  // namespace wwav::py
