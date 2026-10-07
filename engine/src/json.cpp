#include "json.h"

#include <locale.h>
#include <math.h>
#include <stdlib.h>
#if defined(__APPLE__)
#include <xlocale.h>
#endif

namespace wwav::json {

namespace {

constexpr size_t kPythonIntDigits = 4300;

void appendUtf8(std::string* out, uint32_t cp) {
  if (cp < 0x80) {
    out->push_back((char)cp);
  } else if (cp < 0x800) {
    out->push_back((char)(0xC0 | cp >> 6));
    out->push_back((char)(0x80 | (cp & 0x3F)));
  } else if (cp < 0x10000) {
    out->push_back((char)(0xE0 | cp >> 12));
    out->push_back((char)(0x80 | (cp >> 6 & 0x3F)));
    out->push_back((char)(0x80 | (cp & 0x3F)));
  } else {
    out->push_back((char)(0xF0 | cp >> 18));
    out->push_back((char)(0x80 | (cp >> 12 & 0x3F)));
    out->push_back((char)(0x80 | (cp >> 6 & 0x3F)));
    out->push_back((char)(0x80 | (cp & 0x3F)));
  }
}

int hexValue(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

class Parser {
 public:
  Parser(const char* p, size_t n, Dialect d, Handler* h) : p_(p), e_(p + n), python_(d == Dialect::Python), h_(h) {}

  bool run(size_t maxDepth) {
    std::string stack;  // '{' or '[' for each open container
    ws();
    if (p_ == e_ || *p_ != '{') return false;
    for (;;) {
      // A value is due.
      ws();
      if (p_ == e_) return false;
      const char c = *p_;
      if (c == '{' || c == '[') {
        if (maxDepth && stack.size() >= maxDepth) return false;
        p_++;
        stack.push_back(c);
        c == '{' ? h_->beginObject() : h_->beginArray();
        ws();
        if (p_ < e_ && *p_ == (c == '{' ? '}' : ']')) {
          p_++;
          stack.pop_back();
          h_->end();
        } else {
          if (c == '{' && !key()) return false;
          continue;
        }
      } else if (!scalar()) {
        return false;
      }
      // A value is done: close what it ends, up to the next value.
      for (;;) {
        ws();
        if (stack.empty()) return p_ == e_;
        if (p_ == e_) return false;
        if (*p_ == ',') {
          p_++;
          if (stack.back() == '{' && !key()) return false;
          break;
        }
        if (*p_ != (stack.back() == '{' ? '}' : ']')) return false;
        p_++;
        stack.pop_back();
        h_->end();
      }
    }
  }

 private:
  void ws() {
    while (p_ < e_ && (*p_ == ' ' || *p_ == '\t' || *p_ == '\n' || *p_ == '\r')) p_++;
  }

  bool literal(const char* word) {
    const char* q = p_;
    for (; *word; word++, q++)
      if (q == e_ || *q != *word) return false;
    p_ = q;
    return true;
  }

  bool key() {
    ws();
    if (p_ == e_ || *p_ != '"' || !string(&text_)) return false;
    h_->key(text_);
    ws();
    if (p_ == e_ || *p_ != ':') return false;
    p_++;
    return true;
  }

  bool scalar() {
    switch (*p_) {
      case '"':
        if (!string(&text_)) return false;
        h_->string(text_);
        return true;
      case 't':
        if (!literal("true")) return false;
        h_->boolean(true);
        return true;
      case 'f':
        if (!literal("false")) return false;
        h_->boolean(false);
        return true;
      case 'n':
        if (!literal("null")) return false;
        h_->null();
        return true;
      case 'N':
        if (!python_ || !literal("NaN")) return false;
        h_->nonFinite(NAN);
        return true;
      case 'I':
        if (!python_ || !literal("Infinity")) return false;
        h_->nonFinite(INFINITY);
        return true;
      case '-':
        if (python_ && literal("-Infinity")) {
          h_->nonFinite(-INFINITY);
          return true;
        }
        return number();
      default:
        return number();
    }
  }

  bool digits() {
    const char* start = p_;
    while (p_ < e_ && *p_ >= '0' && *p_ <= '9') p_++;
    return p_ > start;
  }

  // -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?
  bool number() {
    const char* start = p_;
    if (*p_ == '-') p_++;
    if (p_ == e_ || *p_ < '0' || *p_ > '9') return false;
    if (*p_ == '0')
      p_++;
    else
      digits();
    const size_t intDigits = (size_t)(p_ - start) - (*start == '-');
    bool integral = true;
    if (p_ < e_ && *p_ == '.') {
      p_++;
      if (!digits()) return false;
      integral = false;
    }
    if (p_ < e_ && (*p_ == 'e' || *p_ == 'E')) {
      p_++;
      if (p_ < e_ && (*p_ == '+' || *p_ == '-')) p_++;
      if (!digits()) return false;
      integral = false;
    }
    // Python turns an integer into an int, which refuses more than 4300 digits.
    if (python_ && integral && intDigits > kPythonIntDigits) return false;
    h_->number(start, (size_t)(p_ - start), integral);
    return true;
  }

  bool hex4(uint32_t* out) {
    if (e_ - p_ < 4) return false;
    uint32_t v = 0;
    for (int i = 0; i < 4; i++) {
      const int d = hexValue(p_[i]);
      if (d < 0) return false;
      v = v << 4 | (uint32_t)d;
    }
    p_ += 4;
    *out = v;
    return true;
  }

  // At the opening quote. The text is already known to be valid UTF-8.
  bool string(std::string* out) {
    out->clear();
    p_++;
    for (;;) {
      if (p_ == e_) return false;
      const unsigned char c = (unsigned char)*p_++;
      if (c == '"') return true;
      if (c < 0x20) return false;  // a control character must be escaped
      if (c != '\\') {
        out->push_back((char)c);
        continue;
      }
      if (p_ == e_) return false;
      switch (*p_++) {
        case '"':
          out->push_back('"');
          break;
        case '\\':
          out->push_back('\\');
          break;
        case '/':
          out->push_back('/');
          break;
        case 'b':
          out->push_back('\b');
          break;
        case 'f':
          out->push_back('\f');
          break;
        case 'n':
          out->push_back('\n');
          break;
        case 'r':
          out->push_back('\r');
          break;
        case 't':
          out->push_back('\t');
          break;
        case 'u': {
          uint32_t cp;
          if (!hex4(&cp)) return false;
          if (cp >= 0xD800 && cp <= 0xDBFF && e_ - p_ >= 6 && p_[0] == '\\' && p_[1] == 'u') {
            const char* back = p_;
            p_ += 2;
            uint32_t low;
            if (hex4(&low) && low >= 0xDC00 && low <= 0xDFFF)
              cp = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
            else
              p_ = back;  // not a pair: the next escape stands on its own
          }
          if (cp >= 0xD800 && cp <= 0xDFFF && !python_) return false;
          appendUtf8(out, cp);
          break;
        }
        default:
          return false;
      }
    }
  }

  const char* p_;
  const char* const e_;
  const bool python_;
  Handler* const h_;
  std::string text_;
};

}  // namespace

bool validUtf8(const char* text, size_t n) {
  const unsigned char* s = (const unsigned char*)text;
  for (size_t i = 0; i < n;) {
    const unsigned char c = s[i];
    if (c < 0x80) {
      i++;
      continue;
    }
    size_t len;
    unsigned char lo = 0x80, hi = 0xBF;  // the allowed range of the second byte
    if (c >= 0xC2 && c <= 0xDF) {
      len = 2;
    } else if (c >= 0xE0 && c <= 0xEF) {
      len = 3;
      if (c == 0xE0) lo = 0xA0;  // overlong
      if (c == 0xED) hi = 0x9F;  // surrogates
    } else if (c >= 0xF0 && c <= 0xF4) {
      len = 4;
      if (c == 0xF0) lo = 0x90;  // overlong
      if (c == 0xF4) hi = 0x8F;  // past U+10FFFF
    } else {
      return false;
    }
    if (n - i < len || s[i + 1] < lo || s[i + 1] > hi) return false;
    for (size_t k = 2; k < len; k++)
      if (s[i + k] < 0x80 || s[i + k] > 0xBF) return false;
    i += len;
  }
  return true;
}

double toDouble(const char* text, size_t n) {
  // strtod reads the locale's decimal point, and JUCE switches the locale
  // for a moment on another thread now and then, so read in "C".
  static const locale_t c = newlocale(LC_ALL_MASK, "C", (locale_t)0);
  const std::string s(text, n);
  return strtod_l(s.c_str(), nullptr, c);
}

bool parseObject(const char* text, size_t n, Dialect dialect, size_t maxDepth, Handler* handler) {
  if (!validUtf8(text, n)) return false;
  Handler none;
  return Parser(text, n, dialect, handler ? handler : &none).run(maxDepth);
}

}  // namespace wwav::json
