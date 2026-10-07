# wwav-engine and wwav-scan (docs/ENGINE.md, docs/SPEC.md 9.1): JUCE 8 with
# PRANA's core. Build:
#
#   bash tools/fetch_juce.sh
#   cmake -S engine -B engine/build -G Ninja -DCMAKE_BUILD_TYPE=Release
#   ninja -C engine/build && (cd engine/build && ctest --output-on-failure)
cmake_minimum_required(VERSION 3.22)

# macOS 13 is the minimum (docs/QUESTIONS.md #50). It must be set before project().
set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING "Minimum macOS")

project(wwav_engine VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(WWAV_FORMATS ${CMAKE_CURRENT_SOURCE_DIR}/../formats CACHE PATH "Mi-WWAV at its pinned commit (tools/bootstrap.sh)")
set(WWAV_JUCE ${CMAKE_CURRENT_SOURCE_DIR}/third_party/JUCE CACHE PATH "JUCE 8.0.15 (tools/fetch_juce.sh)")
if(NOT EXISTS ${WWAV_FORMATS}/prana/core/base/base.h)
  message(FATAL_ERROR "formats/ is empty: run tools/bootstrap.sh (or tools/local_formats.sh in a worktree).")
endif()
if(NOT EXISTS ${WWAV_JUCE}/CMakeLists.txt)
  message(FATAL_ERROR "JUCE is missing: run tools/fetch_juce.sh.")
endif()

# ---- PRANA's core -------------------------------------------------------------
# Built as PRANA builds it (formats/prana/CMakeLists.txt), with the flags that
# make every build compute the same floats.
set(PRANA_CORE_FLAGS -fno-exceptions -fno-rtti -ffp-contract=off -fno-math-errno -fno-fast-math)
file(GLOB_RECURSE PRANA_CORE_SRC CONFIGURE_DEPENDS ${WWAV_FORMATS}/prana/core/*.cpp)
add_library(prana_core STATIC ${PRANA_CORE_SRC})
target_include_directories(prana_core PUBLIC ${WWAV_FORMATS}/prana/core)
target_compile_options(prana_core PRIVATE ${PRANA_CORE_FLAGS} -Wall -Wextra -Wno-unused-parameter)

# ---- The parts that need no JUCE ----------------------------------------------
# Shared memory, media reading, the graph and the WAV writer. The unit tests
# link this alone. The graph's sums and gains must round the same on arm64
# and x86_64, so nothing here may fuse a multiply and an add.
add_library(wwav_engine_core STATIC
  src/graph.cpp
  src/media.cpp
  src/shm.cpp
  src/wav.cpp)
target_include_directories(wwav_engine_core PUBLIC include src)
target_link_libraries(wwav_engine_core PUBLIC prana_core)
target_compile_options(wwav_engine_core PRIVATE -ffp-contract=off -fno-fast-math -Wall -Wextra -Wshadow)
find_package(Threads REQUIRED)
target_link_libraries(wwav_engine_core PUBLIC Threads::Threads)
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  target_link_libraries(wwav_engine_core PUBLIC rt)
endif()

# ---- JUCE ---------------------------------------------------------------------
add_subdirectory(${WWAV_JUCE} JUCE EXCLUDE_FROM_ALL)

# On macOS this is an app bundle with LSUIElement, so it has no Dock icon and
# plugin windows belong to a real application; the app embeds it as
# Contents/Helpers/wwav-engine.app. Elsewhere it is a plain executable.
juce_add_gui_app(wwav-engine
  PRODUCT_NAME "wwav-engine"
  COMPANY_NAME "Wi_WWAV"
  BUNDLE_ID "com.wi-wwav.engine"
  VERSION ${PROJECT_VERSION}
  PLIST_TO_MERGE "<plist><dict><key>LSUIElement</key><true/></dict></plist>")

target_sources(wwav-engine PRIVATE
  src/main.cpp
  src/args.cpp
  src/audio.cpp
  src/build.cpp
  src/engine.cpp
  src/server.cpp)
target_compile_definitions(wwav-engine PRIVATE
  JUCE_WEB_BROWSER=0
  JUCE_USE_CURL=0
  JUCE_JACK=0
  JUCE_DISPLAY_SPLASH_SCREEN=0
  WWAV_ENGINE_VERSION="${PROJECT_VERSION}")
target_compile_options(wwav-engine PRIVATE -ffp-contract=off -fno-fast-math)
target_link_libraries(wwav-engine
  PRIVATE
    wwav_engine_core
    juce::juce_core
    juce::juce_events
    juce::juce_audio_basics
    juce::juce_audio_devices
    juce::juce_cryptography
  PUBLIC
    juce::juce_recommended_config_flags
    juce::juce_recommended_warning_flags)

# Plugin scanning is a later stage (docs/SPEC.md 5.7, 9.4). Until then this
# only says what it will do.
add_executable(wwav-scan scan/main.cpp)

# ---- Tests --------------------------------------------------------------------
enable_testing()

add_executable(wwav-engine-unit tests/unit/unit.cpp)
target_link_libraries(wwav-engine-unit PRIVATE wwav_engine_core)
target_compile_options(wwav-engine-unit PRIVATE -ffp-contract=off -Wall -Wextra)
add_test(NAME engine_unit COMMAND wwav-engine-unit)

find_package(Python3 COMPONENTS Interpreter REQUIRED)
foreach(suite protocol playback render restart streaming)
  add_test(NAME engine_${suite}
    COMMAND ${Python3_EXECUTABLE} -B -m unittest -v test_${suite}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/tests)
  set_tests_properties(engine_${suite} PROPERTIES
    TIMEOUT 300
    ENVIRONMENT "WWAV_ENGINE=$<TARGET_FILE:wwav-engine>;WWAV_PACK=${WWAV_FORMATS}/prana/tools/wwav_pack.py")
endforeach()
add_test(NAME engine_scan_usage COMMAND wwav-scan)
set_tests_properties(engine_scan_usage PROPERTIES WILL_FAIL TRUE)
