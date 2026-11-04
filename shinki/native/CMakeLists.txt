set(_sources NativeOwner.cpp NativeSession.cpp NativeInput.cpp)
if(IOS OR KRKR2_SHINKI_EMU_STATIC)
    add_library(shinki_emu_krkr2 STATIC ${_sources})
    target_compile_definitions(shinki_emu_krkr2 PRIVATE SHINKI_EMU_STATIC_CORE=1)
else()
    add_library(shinki_emu_krkr2 SHARED ${_sources})
endif()
target_include_directories(shinki_emu_krkr2 PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(shinki_emu_krkr2 PRIVATE krkr2plugin krkr2core)
if(IOS)
    enable_language(OBJCXX)
    target_sources(shinki_emu_krkr2 PRIVATE EmbeddedViewIOS.mm)
    set_source_files_properties(EmbeddedViewIOS.mm PROPERTIES COMPILE_OPTIONS "-fobjc-arc")
    target_link_libraries(shinki_emu_krkr2 PRIVATE "-framework OpenGLES" "-framework Foundation" "-framework UIKit")
elseif(ANDROID)
    target_sources(shinki_emu_krkr2 PRIVATE AndroidHost.cpp)
    target_link_libraries(shinki_emu_krkr2 PRIVATE EGL GLESv2)
endif()
set_target_properties(shinki_emu_krkr2 PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/$<0:>"
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/$<0:>"
    ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/$<0:>")
if(IOS OR KRKR2_SHINKI_EMU_STATIC)
    add_library(shinki_krkr2_link_plan SHARED EXCLUDE_FROM_ALL ${_sources})
    target_link_libraries(shinki_krkr2_link_plan PRIVATE
        "$<TARGET_GENEX_EVAL:shinki_emu_krkr2,$<TARGET_PROPERTY:shinki_emu_krkr2,LINK_LIBRARIES>>")
    target_link_options(shinki_krkr2_link_plan PRIVATE
        "$<TARGET_GENEX_EVAL:shinki_emu_krkr2,$<TARGET_PROPERTY:shinki_emu_krkr2,LINK_OPTIONS>>")
endif()
add_custom_target(shinki_emu_krkr2_resources
    COMMAND ${CMAKE_COMMAND} -E copy_directory
        "${PROJECT_SOURCE_DIR}/ui/cocos-studio" "$<TARGET_FILE_DIR:shinki_emu_krkr2>/Resources"
    VERBATIM)
add_dependencies(shinki_emu_krkr2 shinki_emu_krkr2_resources)
add_library(shinki_emu_krkr2_lifecycle_test STATIC NativeLifecycleTest.cpp)
target_include_directories(shinki_emu_krkr2_lifecycle_test PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
set_target_properties(shinki_emu_krkr2_lifecycle_test PROPERTIES
    ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/$<0:>")
add_dependencies(shinki_emu_krkr2 shinki_emu_krkr2_lifecycle_test)
