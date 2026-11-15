cmake_minimum_required(VERSION 3.25)
project(platform_linux_1_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(KF6CoreAddons REQUIRED)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR} COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_linux_1 main.cpp
 ${TELEGRAM_ROOT}/lib_base/base/platform/base_platform_info.cpp
 ${TELEGRAM_ROOT}/lib_base/base/platform/linux/base_linux_library.cpp
 ${TELEGRAM_ROOT}/lib_base/base/algorithm.cpp
 ${TELEGRAM_ROOT}/lib_base/base/debug_log.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_linux_1 PRIVATE ${CMAKE_CURRENT_BINARY_DIR} ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_linux_1 PRIVATE -O1 -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_linux_1 PRIVATE -Wl,--gc-sections)
target_link_libraries(native_linux_1 PRIVATE Qt6::Core KF6::CoreAddons ${CMAKE_DL_LIBS})
