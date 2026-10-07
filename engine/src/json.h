#pragma once
// JSON without recursion, so no payload can overflow a stack however deep it
// nests. Two dialects:
//
// - Strict: RFC 8259, for the command socket (docs/ENGINE.md 2). The text
//   is valid UTF-8, one value with only whitespace around it, no trailing
//   commas, no comments, and every \u escape of a surrogate is half of a
//   pair (as I-JSON, RFC 7493, asks).
// - Python: what the reference reader (formats/prana/tools/wwav_pack.py)
//   accepts when it reads a .wwav's wmet and wlin with Python's json module:
//   the same, plus NaN, Infinity and -Infinity, lone surrogate escapes, and
//   integers of at most 4300 digits (Python's int limit, sys.int_info).
#include <stddef.h>
#include <stdint.h>

#include <string>

namespace wwav::json {

enum class Dialect { Strict, Python };

// What the parser finds, in document order. Strings arrive decoded, as
// UTF-8 (a lone surrogate, which only Python allows, as its 3-byte form);
// they may hold U+0000.
class Handler {
 public:
  virtual ~Handler() = default;
  virtual void beginObject() {}
  virtual void beginArray() {}
  virtual void end() {}  // of the innermost object or array
  virtual void key(const std::string&) {}
  virtual void string(const std::string&) {}
  // The number's text as written; integral when it has no fraction or exponent.
  virtual void number(const char*, size_t, bool /*integral*/) {}
  virtual void boolean(bool) {}
  virtual void null() {}
  // Python's NaN, Infinity and -Infinity.
  virtual void nonFinite(double) {}
};

// True when [text, text + n) is one JSON object in `dialect` that nests at
// most `maxDepth` levels (0: no limit). The handler may have been called
// before a false.
bool parseObject(const char* text, size_t n, Dialect dialect, size_t maxDepth, Handler* handler);

// Whether [text, text + n) is valid UTF-8 (RFC 3629: no overlong forms, no
// surrogates, nothing past U+10FFFF).
bool validUtf8(const char* text, size_t n);

// A JSON number's text as the nearest double, whatever the process's locale.
double toDouble(const char* text, size_t n);

}  // namespace wwav::json
