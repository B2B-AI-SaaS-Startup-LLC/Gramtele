cmake_minimum_required(VERSION 3.25)
project(calls_notify_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
get_target_property(QT_CORE_INCLUDES Qt6::Core INTERFACE_INCLUDE_DIRECTORIES)
find_library(QT_GUI_LIBRARY NAMES Qt6Gui PATHS ${Qt6_DIR}/../.. REQUIRED NO_DEFAULT_PATH)
include_directories(${Qt6_DIR}/../../../include/QtGui)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR} COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_calls_notify main.cpp ${CMAKE_CURRENT_BINARY_DIR}/chart.cpp ${CMAKE_CURRENT_BINARY_DIR}/parse.cpp
 ${TELEGRAM_ROOT}/SourceFiles/data/raw/raw_countries_bounds.cpp
 ${TELEGRAM_ROOT}/SourceFiles/statistics/segment_tree.cpp
 ${TELEGRAM_ROOT}/lib_base/base/algorithm.cpp ${TELEGRAM_ROOT}/lib_base/base/debug_log.cpp ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_calls_notify PRIVATE ${CMAKE_CURRENT_BINARY_DIR} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_calls_notify PRIVATE -O1 -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_calls_notify PRIVATE -Wl,--gc-sections)
target_link_libraries(native_calls_notify PRIVATE Qt6::Core ${QT_GUI_LIBRARY})
set_source_files_properties(${TELEGRAM_ROOT}/lib_base/base/debug_log.cpp PROPERTIES COMPILE_OPTIONS "-include;crl/crl_time.h")
