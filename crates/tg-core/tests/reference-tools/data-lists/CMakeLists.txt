cmake_minimum_required(VERSION 3.25)
project(data_lists_reference LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 23)
find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/extract.py ${TELEGRAM_ROOT} ${CMAKE_CURRENT_BINARY_DIR} COMMAND_ERROR_IS_FATAL ANY)
add_executable(native_lists main.cpp ${CMAKE_CURRENT_BINARY_DIR}/premium.cpp
 ${TELEGRAM_ROOT}/SourceFiles/data/data_messages.cpp
 ${TELEGRAM_ROOT}/SourceFiles/data/data_sparse_ids.cpp
 ${TELEGRAM_ROOT}/SourceFiles/data/data_peer_id.cpp
 ${SCHEME_INCLUDE}/scheme.cpp
 ${TELEGRAM_ROOT}/lib_tl/tl/tl_basic_types.cpp
 ${TELEGRAM_ROOT}/lib_base/base/algorithm.cpp
 ${TELEGRAM_ROOT}/lib_base/base/debug_log.cpp
 ${TELEGRAM_ROOT}/lib_base/base/assertion.cpp
 ${TELEGRAM_ROOT}/lib_base/base/integration.cpp)
target_include_directories(native_lists PRIVATE ${CMAKE_CURRENT_BINARY_DIR} ${SCHEME_INCLUDE} ${TELEGRAM_ROOT}/SourceFiles ${TELEGRAM_ROOT}/lib_tl ${TELEGRAM_ROOT}/lib_ui ${TELEGRAM_ROOT}/lib_base ${TELEGRAM_ROOT}/lib_rpl ${TELEGRAM_ROOT}/lib_crl ${GSL_INCLUDE} ${RANGE_INCLUDE})
target_compile_options(native_lists PRIVATE -O1 -include ${CMAKE_CURRENT_SOURCE_DIR}/prefix.hpp -ffunction-sections -fdata-sections)
target_link_options(native_lists PRIVATE -Wl,--gc-sections -Wl,--wrap=_ZNK4Data13PremiumLimits14appConfigLimitERK7QStringi -Wl,--wrap=_ZNK4Data13PremiumLimits9isPremiumEv -Wl,--wrap=_ZN4Data16ForMessagesToMTPERKNS_18BusinessRecipientsE)
target_link_libraries(native_lists PRIVATE Qt6::Core)
set_source_files_properties(${TELEGRAM_ROOT}/lib_base/base/debug_log.cpp PROPERTIES COMPILE_OPTIONS "-include;crl/crl_time.h")
