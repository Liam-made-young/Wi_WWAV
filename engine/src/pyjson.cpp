#include "pyjson.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include "json.h"

namespace wwav::py {

namespace {

// Each run of ten decimal digits Unicode has (category Nd), by its zero:
// what Python's \d and int() take. Unicode 15.1, as Python 3.13 has it.
constexpr uint32_t kDigitZeros[] = {
    0x30,    0x660,   0x6F0,   0x7C0,   0x966,   0x9E6,   0xA66,   0xAE6,   0xB66,   0xBE6,   0xC66,   0xCE6,
    0xD66,   0xDE6,   0xE50,   0xED0,   0xF20,   0x1040,  0x1090,  0x17E0,  0x1810,  0x1946,  0x19D0,  0x1A80,
    0x1A90,  0x1B50,  0x1BB0,  0x1C40,  0x1C50,  0xA620,  0xA8D0,  0xA900,  0xA9D0,  0xA9F0,  0xAA50,  0xABF0,
    0xFF10,  0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0,
    0x11730, 0x118E0, 0x11950, 0x11C50, 0x11D50, 0x11DA0, 0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8,
    0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950, 0x1FBF0};

// Code points past ASCII that repr() escapes because str.isprintable() says
// no (controls, separators, format characters, surrogates, private use).
// Unicode 15.1. Code points Unicode hasn't assigned, which Python escapes
// too, are left as they are: that table is 700 ranges long, and the only
// place repr() matters is a list or object nested in wmet's frames.
constexpr uint32_t kUnprintable[][2] = {{0x80, 0xA0},       {0xAD, 0xAD},       {0x600, 0x605},      {0x61C, 0x61C},
                                        {0x6DD, 0x6DD},     {0x70F, 0x70F},     {0x890, 0x891},      {0x8E2, 0x8E2},
                                        {0x1680, 0x1680},   {0x180E, 0x180E},   {0x2000, 0x200F},    {0x2028, 0x202F},
                                        {0x205F, 0x2064},   {0x2066, 0x206F},   {0x3000, 0x3000},    {0xD800, 0xF8FF},
                                        {0xFEFF, 0xFEFF},   {0xFFF9, 0xFFFB},   {0x110BD, 0x110BD},  {0x110CD, 0x110CD},
                                        {0x13430, 0x1343F}, {0x1BCA0, 0x1BCA3}, {0x1D173, 0x1D17A},  {0xE0001, 0xE0001},
                                        {0xE0020, 0xE007F}, {0xF0000, 0xFFFFD}, {0x100000, 0x10FFFD}};

// The code point at `i` of UTF-8 (a lone surrogate's 3-byte form included);
// moves `i` past it. The text is valid, so no checks.
uint32_t next(const std::string& s, size_t* i) {
  const unsigned char c = (unsigned char)s[*i];
  const int len = c < 0x80 ? 1 : c < 0xE0 ? 2 : c < 0xF0 ? 3 : 4;
  uint32_t cp = len == 1 ? c : len == 2 ? c & 0x1F : len == 3 ? c & 0x0F : c & 0x07;
  for (int k = 1; k < len; k++) cp = cp << 6 | ((unsigned char)s[*i + k] & 0x3F);
  *i += (size_t)len;
  return cp;
}

int digitValue(uint32_t cp) {
  for (uint32_t zero : kDigitZeros)
    if (cp >= zero && cp < zero + 10) return (int)(cp - zero);
  return -1;
}

bool printable(uint32_t cp) {
  if (cp < 0x80) return cp >= 0x20 && cp < 0x7F;
  for (const auto& r : kUnprintable)
    if (cp >= r[0] && cp <= r[1]) return false;
  return true;
}

// repr() of a str.
std::string reprStr(const std::string& s) {
  const bool single = s.find('\'') != std::string::npos, dbl = s.find('"') != std::string::npos;
  const char quote = single && !dbl ? '"' : '\'';
  std::string out(1, quote);
  char buf[16];
  for (size_t i = 0; i < s.size();) {
    const size_t at = i;
    const uint32_t cp = next(s, &i);
    if (cp == '\\' || cp == (uint32_t)quote) {
      out += '\\';
      out += (char)cp;
    } else if (cp == '\t') {
      out += "\\t";
    } else if (cp == '\n') {
      out += "\\n";
    } else if (cp == '\r') {
      out += "\\r";
    } else if (printable(cp)) {
      out.append(s, at, i - at);
    } else {
      snprintf(buf, sizeof buf, cp < 0x100 ? "\\x%02x" : cp < 0x10000 ? "\\u%04x" : "\\U%08x", cp);
      out += buf;
    }
  }
  return out + quote;
}

// str() of an int written in JSON: as written, but for -0.
std::string intStr(const std::string& text) { return text == "-0" ? "0" : text; }

class Reader final : public json::Handler {
 public:
  Reader(const std::vector<std::string>& keys, std::vector<Value>* values) : keys_(keys), values_(values) {}

  void beginObject() override { open('}'); }
  void beginArray() override { open(']'); }
  void end() override {
    depth_--;
    if (!capturing_) return;
    repr_ += closers_.back();
    closers_.pop_back();
    counts_.pop_back();
    if (depth_ == 1) {
      capturing_ = false;
      Value& v = (*values_)[(size_t)slot_];
      v.str = repr_;
    }
  }
  void key(const std::string& k) override {
    if (depth_ == 1) {
      slot_ = -1;
      for (size_t i = 0; i < keys_.size(); i++)
        if (keys_[i] == k) slot_ = (int)i;
      return;
    }
    if (!capturing_) return;
    if (counts_.back()++ > 0) repr_ += ", ";
    repr_ += reprStr(k) + ": ";
  }
  void string(const std::string& s) override {
    Value v;
    v.kind = Value::Str;
    v.text = s;
    v.str = s;
    scalar(v, reprStr(s));
  }
  void number(const char* p, size_t n, bool integral) override {
    Value v;
    if (integral) {
      v.kind = Value::Int;
      v.text.assign(p, n);
      v.str = intStr(v.text);
    } else {
      v.kind = Value::Float;
      v.f = json::toDouble(p, n);
      v.str = floatRepr(v.f);
    }
    scalar(v, v.str);
  }
  void nonFinite(double d) override {
    Value v;
    v.kind = Value::Float;
    v.f = d;
    v.str = floatRepr(d);
    scalar(v, v.str);
  }
  void boolean(bool b) override {
    Value v;
    v.kind = Value::Bool;
    v.b = b;
    v.str = b ? "True" : "False";
    scalar(v, v.str);
  }
  void null() override {
    Value v;
    v.kind = Value::None;
    v.str = "None";
    scalar(v, v.str);
  }

 private:
  void open(char closer) {
    if (depth_++ == 0) return;  // the object itself
    if (depth_ == 2) {
      // A member that is a list or an object: its repr is built as it is read.
      if (slot_ < 0) return;
      Value& v = (*values_)[(size_t)slot_];
      v = Value();
      v.kind = closer == '}' ? Value::Dict : Value::List;
      capturing_ = true;
      repr_.clear();
    } else if (!capturing_) {
      return;
    } else if (closers_.back() == ']' && counts_.back()++ > 0) {
      repr_ += ", ";
    }
    repr_ += closer == '}' ? '{' : '[';
    closers_.push_back(closer);
    counts_.push_back(0);
  }

  void scalar(const Value& v, const std::string& repr) {
    if (depth_ == 1) {
      if (slot_ >= 0) (*values_)[(size_t)slot_] = v;
      return;
    }
    if (!capturing_) return;
    if (closers_.back() == ']' && counts_.back()++ > 0) repr_ += ", ";
    repr_ += repr;
  }

  const std::vector<std::string>& keys_;
  std::vector<Value>* values_;
  size_t depth_ = 0;
  int slot_ = -1;  // the wanted key the next member's value belongs to
  bool capturing_ = false;
  std::string repr_;
  std::string closers_;
  std::vector<size_t> counts_;  // items so far in each open list or object
};

}  // namespace

bool readObject(const std::string& payload, const std::vector<std::string>& keys, std::vector<Value>* values) {
  values->assign(keys.size(), Value());
  Reader reader(keys, values);
  return json::parseObject(payload.data(), payload.size(), json::Dialect::Python, 0, &reader);
}

std::string floatRepr(double d) {
  if (isnan(d)) return "nan";
  if (isinf(d)) return d > 0 ? "inf" : "-inf";
  const std::string sign = signbit(d) ? "-" : "";
  d = fabs(d);
  if (d == 0.0) return sign + "0.0";
  // The fewest significant digits that read back as d (printf rounds
  // correctly), then laid out as Python's repr lays them out.
  std::string digits;
  int exp10 = 0;
  for (int p = 1; p <= 17; p++) {
    char buf[40];
    snprintf(buf, sizeof buf, "%.*e", p - 1, d);
    digits.clear();
    const char* q = buf;
    for (; *q && *q != 'e'; q++)
      if (*q >= '0' && *q <= '9') digits += *q;  // whatever the locale's point
    exp10 = atoi(q + 1);
    const std::string back = digits.substr(0, 1) + "." + digits.substr(1) + "e" + std::to_string(exp10);
    if (json::toDouble(back.data(), back.size()) == d) break;
  }
  while (digits.size() > 1 && digits.back() == '0') digits.pop_back();
  const int point = exp10 + 1;  // d is 0.<digits> times 10^point
  std::string out;
  if (point <= -4 || point > 16) {
    out = digits.substr(0, 1);
    if (digits.size() > 1) out += "." + digits.substr(1);
    char e[16];
    snprintf(e, sizeof e, "e%c%02d", exp10 < 0 ? '-' : '+', abs(exp10));
    out += e;
  } else if (point <= 0) {
    out = "0." + std::string((size_t)-point, '0') + digits;
  } else if ((size_t)point >= digits.size()) {
    out = digits + std::string((size_t)point - digits.size(), '0') + ".0";
  } else {
    out = digits.substr(0, (size_t)point) + "." + digits.substr((size_t)point);
  }
  return sign + out;
}

bool leadingDigits(const std::string& s, bool* positive) {
  bool any = false;
  *positive = false;
  for (size_t i = 0; i < s.size();) {
    const int d = digitValue(next(s, &i));
    if (d < 0) break;
    any = true;
    *positive = *positive || d > 0;
  }
  return any;
}

bool equals(const Value& v, uint64_t n) {
  switch (v.kind) {
    case Value::Int:
      return intStr(v.text) == std::to_string(n);
    case Value::Float:
      return v.f == (double)n;
    case Value::Bool:
      return (v.b ? 1u : 0u) == n;
    default:
      return false;
  }
}

}  // namespace wwav::py
