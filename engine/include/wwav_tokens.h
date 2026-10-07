// Generated from design/tokens.json by tools/tokens/compile.mjs.
// Don't edit it: change the JSON and run the compiler again.

#ifndef WWAV_TOKENS_H
#define WWAV_TOKENS_H

#include <cstddef>
#include <cstdint>

namespace wwav::tokens {

// An sRGB colour; a is 1 except for the few translucent tokens.
struct Color {
  std::uint8_t r, g, b;
  float a;
};

// A token with its own value in light and dark.
template <typename T>
struct Themed {
  T light, dark;
};

// A gradient's stops (top or centre first), or a list of names or numbers.
template <typename T>
struct List {
  const T* items;
  std::size_t count;
};

// A CSS cubic-bezier() timing curve.
struct Curve {
  float x1, y1, x2, y2;
};

namespace desk {
// Heat and the Console's chrome, Settings and every sheet (8.2). Light and
// dark are Heat's tokens with 8.2's five fixes.
inline constexpr Themed<Color> kInk{{0x1b, 0x1b, 0x1b, 1.0f}, {0xec, 0xec, 0xec, 1.0f}};
inline constexpr Themed<Color> kInk2{{0x4a, 0x4a, 0x4a, 1.0f}, {0xc2, 0xc2, 0xc2, 1.0f}};
inline constexpr Themed<Color> kInk3{{0x5f, 0x5f, 0x5f, 1.0f}, {0x9a, 0x9a, 0x9a, 1.0f}};
inline constexpr Themed<Color> kWell{{0xff, 0xff, 0xff, 1.0f}, {0x26, 0x28, 0x2b, 1.0f}};
inline constexpr Themed<Color> kStripe{{0xed, 0xf3, 0xfe, 1.0f}, {0x2c, 0x30, 0x38, 1.0f}};
inline constexpr Color kCaseMetalLight[] = {{0xe4, 0xe4, 0xe4, 1.0f}, {0xc9, 0xc9, 0xc9, 1.0f}};
inline constexpr Color kCaseMetalDark[] = {{0x5a, 0x5d, 0x61, 1.0f}, {0x3f, 0x42, 0x46, 1.0f}};
inline constexpr Themed<List<Color>> kCaseMetal{{kCaseMetalLight, 2}, {kCaseMetalDark, 2}};
inline constexpr Color kDeckMetalLight[] = {{0xfb, 0xfc, 0xfd, 1.0f}, {0xe6, 0xea, 0xef, 1.0f}, {0xc9, 0xd0, 0xd8, 1.0f}};
inline constexpr Color kDeckMetalDark[] = {{0x5a, 0x5d, 0x61, 1.0f}, {0x3f, 0x42, 0x46, 1.0f}};
inline constexpr Themed<List<Color>> kDeckMetal{{kDeckMetalLight, 3}, {kDeckMetalDark, 2}};
inline constexpr Themed<Color> kDeckEdge{{0x98, 0xa2, 0xad, 1.0f}, {0x26, 0x28, 0x2b, 1.0f}};
inline constexpr Themed<Color> kDeckPinstripe{{0xe9, 0xef, 0xf6, 1.0f}, {0x00, 0x00, 0x00, 0.0f}};
inline constexpr float kPinstripeEveryPx = 4.0f;
inline constexpr Themed<Color> kDeckInk{{0x16, 0x1d, 0x26, 1.0f}, {0xec, 0xec, 0xec, 1.0f}};
inline constexpr Themed<Color> kDeckInkSoft{{0x5d, 0x69, 0x75, 1.0f}, {0xec, 0xec, 0xec, 1.0f}};
inline constexpr Color kSelect{0x1b, 0x4c, 0x8c, 1.0f};
inline constexpr Color kSelectInk{0xff, 0xff, 0xff, 1.0f};
inline constexpr float kSelectBarPx = 3.0f;
inline constexpr Themed<Color> kHighlight{{0x38, 0x75, 0xd7, 1.0f}, {0x3a, 0x6f, 0xc4, 1.0f}};
inline constexpr Themed<Color> kFocus{{0x38, 0x75, 0xd7, 0.6f}, {0x3a, 0x6f, 0xc4, 0.6f}};
inline constexpr float kFocusWidthPx = 3.0f;
inline constexpr Color kGelSelectedItems[] = {{0x33, 0x6d, 0xcc, 1.0f}, {0x1b, 0x4c, 0x8c, 1.0f}};
inline constexpr List<Color> kGelSelected{kGelSelectedItems, 2};
inline constexpr float kGelPress = 0.88f;
inline constexpr Color kBackdrop{0x00, 0x00, 0x00, 0.25f};

namespace lcd {
// The Now strip: Heat's olive LCD, with its dim text darkened (2.2, 8.2).
inline constexpr Color kGroundLight[] = {{0xf2, 0xf4, 0xe4, 1.0f}, {0xdf, 0xe3, 0xc6, 1.0f}};
inline constexpr Color kGroundDark[] = {{0x2c, 0x31, 0x21, 1.0f}, {0x20, 0x24, 0x1a, 1.0f}};
inline constexpr Themed<List<Color>> kGround{{kGroundLight, 2}, {kGroundDark, 2}};
inline constexpr Themed<Color> kInk{{0x26, 0x2a, 0x17, 1.0f}, {0xd7, 0xe0, 0xa8, 1.0f}};
inline constexpr Themed<Color> kDim{{0x4b, 0x50, 0x34, 1.0f}, {0x8e, 0x96, 0x70, 1.0f}};
}  // namespace lcd

namespace sourceList {
// The library drawer and Heat's sidebar, with headings darkened (8.2).
inline constexpr Themed<Color> kGround{{0xe7, 0xec, 0xf2, 1.0f}, {0x2e, 0x31, 0x36, 1.0f}};
inline constexpr Themed<Color> kHeading{{0x4f, 0x57, 0x61, 1.0f}, {0x9a, 0xa3, 0xad, 1.0f}};
}  // namespace sourceList
}  // namespace desk

namespace screen {
// The Game Boy inside: every Console readout is a DMG screen, the same in
// light and dark (8.2, 5.2; MI-WWAV-OS theme.dart). Meters are DMG green to -6
// dBFS, amber to -1, rec above.

namespace dmg {
inline constexpr Color kDeep{0x0b, 0x1f, 0x0b, 1.0f};
inline constexpr Color kDark{0x0f, 0x38, 0x0f, 1.0f};
inline constexpr Color kMid{0x30, 0x62, 0x30, 1.0f};
inline constexpr Color kLit{0x8b, 0xac, 0x0f, 1.0f};
}  // namespace dmg
inline constexpr Color kGlow{0xc6, 0xe2, 0x4a, 1.0f};
inline constexpr Color kAmber{0xf0, 0xa3, 0x2e, 1.0f};
inline constexpr Color kRec{0xe0, 0x45, 0x3a, 1.0f};
inline constexpr float kMeterAmberFromDb = -6.0f;
inline constexpr float kMeterRecFromDb = -1.0f;
}  // namespace screen

namespace night {
// Space and the expanded player, in both appearances (8.2; v3, v4). Seven ink
// opacities are the only hierarchy scale; royal blue has three uses and is
// never text.
inline constexpr Color kGround{0x07, 0x0a, 0x18, 1.0f};
inline constexpr Color kDeep{0x03, 0x05, 0x10, 1.0f};
inline constexpr Color kClay{0x1a, 0x21, 0x42, 1.0f};
inline constexpr Color kInk{0xf4, 0xef, 0xe6, 1.0f};
inline constexpr float kInkStepsItems[] = {1.0f, 0.7f, 0.5f, 0.35f, 0.2f, 0.12f, 0.07f};
inline constexpr List<float> kInkSteps{kInkStepsItems, 7};

namespace textSteps {
inline constexpr float kBodyItems[] = {1.0f, 0.7f};
inline constexpr List<float> kBody{kBodyItems, 2};
inline constexpr float kSecondaryItems[] = {0.5f};
inline constexpr List<float> kSecondary{kSecondaryItems, 1};
}  // namespace textSteps
inline constexpr Color kStar{0xdd, 0xe4, 0xfb, 1.0f};
inline constexpr Color kAccent{0x29, 0x46, 0xff, 1.0f};
inline constexpr const char* kAccentUsesItems[] = {"armed", "branch", "commit"};
inline constexpr List<const char*> kAccentUses{kAccentUsesItems, 3};
inline constexpr Color kSunItems[] = {{0xff, 0xf8, 0xe6, 1.0f}, {0xff, 0xd8, 0x9a, 1.0f}, {0xf0, 0xa2, 0x3c, 1.0f}, {0xb2, 0x5f, 0x12, 1.0f}};
inline constexpr List<Color> kSun{kSunItems, 4};
inline constexpr float kFocusWidthPx = 2.0f;
}  // namespace night

namespace room {
// Unquantized, lit rather than painted (8.2; v3's mock, PRANA, Crater). Dusk
// is light, after hours is dark: the walls fall to limousine and paper stays
// paper.

namespace field {
inline constexpr Color kGroundLight[] = {{0xf7, 0xf4, 0xee, 1.0f}, {0xe6, 0xdf, 0xd0, 1.0f}};
inline constexpr Color kGroundDark[] = {{0x0f, 0x0c, 0x09, 1.0f}, {0x0f, 0x0c, 0x09, 1.0f}};
inline constexpr Themed<List<Color>> kGround{{kGroundLight, 2}, {kGroundDark, 2}};
inline constexpr Color kRose{0xfb, 0xed, 0xe8, 1.0f};
inline constexpr Color kCool{0xdd, 0xe2, 0xe6, 1.0f};
inline constexpr float kDriftShortestMs = 53000.0f;
inline constexpr float kDriftLongestMs = 131000.0f;
}  // namespace field

namespace material {
inline constexpr Color kSand{0xe8, 0xdc, 0xc8, 1.0f};
inline constexpr Color kSandDeep{0xd4, 0xc4, 0xa8, 1.0f};
inline constexpr Color kClay{0xb8, 0x98, 0x78, 1.0f};
inline constexpr Color kClayDeep{0x7a, 0x5e, 0x45, 1.0f};
}  // namespace material
inline constexpr Color kInk{0x3d, 0x2e, 0x22, 1.0f};
inline constexpr Color kInk2{0x6b, 0x56, 0x43, 1.0f};
inline constexpr Color kPaper{0xff, 0xe9, 0xc8, 1.0f};
inline constexpr Color kAccent{0xc8, 0x96, 0x68, 1.0f};
inline constexpr Color kLampItems[] = {{0xe8, 0x91, 0x5b, 1.0f}, {0xf4, 0xb4, 0x83, 1.0f}};
inline constexpr List<Color> kLamp{kLampItems, 2};
inline constexpr Color kSkyItems[] = {{0xf8, 0xd0, 0xa4, 1.0f}, {0xd4, 0x76, 0x3f, 1.0f}};
inline constexpr List<Color> kSky{kSkyItems, 2};
inline constexpr float kHairlinePx = 1.0f;
inline constexpr float kFocusWidthPx = 2.0f;
}  // namespace room

namespace stem {
// PRANA's stem colours, the same everywhere, in stem order (8.3). Every stem
// mark carries a ring in its register's ink; state is shape: muted is a
// stem-colour ring, soloed adds an outer ring, silent under a solo fills at
// 45% with a dashed ring, selected adds a notch.
inline constexpr Color kVocals{0xd2, 0x3c, 0x2a, 1.0f};
inline constexpr Color kDrums{0xf0, 0xb9, 0x0b, 1.0f};
inline constexpr Color kOther{0x2e, 0x9a, 0x55, 1.0f};
inline constexpr Color kBass{0x1f, 0x4e, 0x9e, 1.0f};
inline constexpr float kRingPx = 1.0f;
inline constexpr float kMutedRingPx = 2.0f;
inline constexpr float kSoloRingPx = 2.0f;
inline constexpr float kSilentFill = 0.45f;
inline constexpr float kNotchPx = 2.0f;
}  // namespace stem

namespace heat {
// Heat's level colours (3.1). They fill tubes, pill borders and dots; the
// level word is ink.
inline constexpr Color kCool{0x4f, 0x9b, 0xe6, 1.0f};
inline constexpr Color kWarm{0xef, 0xa4, 0x31, 1.0f};
inline constexpr Color kHot{0xe0, 0x40, 0x2c, 1.0f};
inline constexpr Color kOverdue{0x8f, 0x1d, 0x16, 1.0f};
}  // namespace heat

namespace label {
// The six colour labels, for sorting only (2.5).
inline constexpr Color kRed{0xe0, 0x38, 0x3e, 1.0f};
inline constexpr Color kOrange{0xf0, 0x8a, 0x2c, 1.0f};
inline constexpr Color kYellow{0xe8, 0xc7, 0x3a, 1.0f};
inline constexpr Color kGreen{0x3c, 0xaa, 0x68, 1.0f};
inline constexpr Color kBlue{0x3d, 0x7d, 0xd5, 1.0f};
inline constexpr Color kPurple{0x93, 0x59, 0xc9, 1.0f};
}  // namespace label

namespace key {
// A work's key colour (8.3, v4 keyColor.js): hue = ((pc * 7) mod 12) * 30 with
// A = 0, in hsl with the mode's saturation and lightness. An unknown key is
// night indigo in the minor look. The glow tone an orb's rim takes is the same
// hue and saturation at the glow lightness.
inline constexpr float kUnknownHue = 232.0f;

namespace major {
inline constexpr float kSaturation = 72.0f;
inline constexpr float kLightness = 58.0f;
}  // namespace major

namespace minor {
inline constexpr float kSaturation = 58.0f;
inline constexpr float kLightness = 42.0f;
}  // namespace minor
inline constexpr float kGlowLightness = 65.0f;
}  // namespace key

namespace light {
// One light, from the upper left, as a fraction of the box (8.3, v3
// WWAVLight.sun).

namespace sun {
inline constexpr float kX = 0.3f;
inline constexpr float kY = 0.25f;
}  // namespace sun
}  // namespace light

namespace motion {
// Built on MI-WWAV-OS's 140 ms beat with v4's curves (8.6). Under Reduce
// Motion every move is a beat-long cross-fade. Nothing bounces.
inline constexpr float kPressMs = 70.0f;
inline constexpr float kBeatMs = 140.0f;
inline constexpr float kFadeMs = 280.0f;
inline constexpr float kMorphMs = 420.0f;
inline constexpr float kWashMs = 760.0f;
inline constexpr float kDiveMs = 900.0f;
inline constexpr float kToastMs = 2600.0f;
inline constexpr Curve kRest{0.22f, 1.0f, 0.36f, 1.0f};
inline constexpr Curve kSheet{0.32f, 0.72f, 0.0f, 1.0f};
}  // namespace motion

namespace space {
// A 4 px grid (8.3).
inline constexpr float kS1Px = 4.0f;
inline constexpr float kS2Px = 8.0f;
inline constexpr float kS3Px = 12.0f;
inline constexpr float kS4Px = 16.0f;
inline constexpr float kS5Px = 24.0f;
inline constexpr float kS6Px = 32.0f;
inline constexpr float kS7Px = 48.0f;
inline constexpr float kS8Px = 64.0f;
}  // namespace space

namespace radius {
// Corners (8.3). Gels are full pills; a sheet rounds only its lower corners.
inline constexpr float kChipPx = 6.0f;
inline constexpr float kButtonPx = 10.0f;
inline constexpr float kCardPx = 14.0f;
inline constexpr float kSheetPx = 8.0f;
inline constexpr float kPillPx = 999.0f;
}  // namespace radius

namespace type {
// Faces and sizes (8.4). Nothing is under 11; text read for more than a line
// is at least the reading size (8.9, open: 17 or 19).

namespace desk {
inline constexpr const char* kFamilyItems[] = {"Lucida Grande", "Lucida Sans Unicode", "Lucida Sans", "sans-serif"};
inline constexpr List<const char*> kFamily{kFamilyItems, 4};
inline constexpr float kSmallPx = 11.0f;
inline constexpr float kBodyPx = 13.0f;
inline constexpr float kLargePx = 15.0f;
inline constexpr float kTitlePx = 20.0f;
inline constexpr float kLineHeight = 1.35f;
}  // namespace desk

namespace mono {
inline constexpr const char* kFamilyItems[] = {"Menlo", "Consolas", "monospace"};
inline constexpr List<const char*> kFamily{kFamilyItems, 3};
inline constexpr float kSmallPx = 11.0f;
inline constexpr float kBodyPx = 13.0f;
inline constexpr float kLargePx = 20.0f;
}  // namespace mono

namespace name {
inline constexpr const char* kFamilyItems[] = {"Cormorant Garamond", "serif"};
inline constexpr List<const char*> kFamily{kFamilyItems, 2};
inline constexpr float kMinPx = 20.0f;
inline constexpr float kHeaderPx = 30.0f;
inline constexpr float kSunPx = 46.0f;
inline constexpr float kLineHeight = 1.18f;
inline constexpr float kWeight = 400.0f;
inline constexpr float kLightWeight = 300.0f;
inline constexpr float kLightWeightFromPx = 28.0f;
}  // namespace name

namespace text {
inline constexpr const char* kFamilyItems[] = {"Inter", "sans-serif"};
inline constexpr List<const char*> kFamily{kFamilyItems, 2};
inline constexpr float kLabelPx = 11.0f;
inline constexpr float kLabelTrackingEm = 0.08f;
inline constexpr float kSmallPx = 13.0f;
inline constexpr float kBodyPx = 15.0f;
inline constexpr float kReadingPx = 17.0f;
}  // namespace text

namespace sign {
inline constexpr const char* kFamilyItems[] = {"Jost", "sans-serif"};
inline constexpr List<const char*> kFamily{kFamilyItems, 2};
inline constexpr float kMinPx = 11.0f;
inline constexpr float kMaxPx = 15.0f;
inline constexpr float kTrackingEm = 0.34f;
}  // namespace sign
}  // namespace type

}  // namespace wwav::tokens

#endif  // WWAV_TOKENS_H
