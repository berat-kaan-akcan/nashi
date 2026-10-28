cmake_minimum_required(VERSION 3.21)

project(nashi VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_AUTOMOC ON)
include(GNUInstallDirs)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Widgets DBus LinguistTools Test)
qt_standard_project_setup(REQUIRES 6.5 I18N_SOURCE_LANGUAGE en I18N_TRANSLATED_LANGUAGES tr)
qt_policy(SET QTP0004 NEW)

# --- LayerShellQt (optional) -------------------------------------------------
find_package(LayerShellQt CONFIG QUIET)
option(NASHI_WITH_LAYERSHELL "Use LayerShellQt for the overlay and toasts on Wayland" ${LayerShellQt_FOUND})
if(NASHI_WITH_LAYERSHELL AND NOT LayerShellQt_FOUND)
    message(FATAL_ERROR "NASHI_WITH_LAYERSHELL=ON but LayerShellQt was not found")
endif()
message(STATUS "Nashi: LayerShellQt support: ${NASHI_WITH_LAYERSHELL}")

if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    add_compile_options(-Wall -Wextra -Wpedantic)
endif()
add_compile_definitions(
    QT_NO_KEYWORDS
    QT_NO_CAST_FROM_ASCII
    QT_DISABLE_DEPRECATED_UP_TO=0x060500
    NASHI_VERSION="${PROJECT_VERSION}")

# --- Core library + QML module (shared by the app and the tests) -------------
qt_add_library(nashi_ui STATIC)
target_sources(nashi_ui PRIVATE
    src/actions.cpp            src/actions.h
    src/app.cpp                src/app.h
    src/appdbus.cpp            src/appdbus.h
    src/cliargs.cpp            src/cliargs.h
    src/clilocator.cpp         src/clilocator.h
    src/depsmodel.cpp          src/depsmodel.h
    src/iconprovider.cpp       src/iconprovider.h
    src/inputdclient.cpp       src/inputdclient.h
    src/jobmanager.cpp         src/jobmanager.h
    src/jobrunner.cpp          src/jobrunner.h
    src/jsonlinebuffer.cpp     src/jsonlinebuffer.h
    src/language.cpp           src/language.h
    src/notifier.cpp           src/notifier.h
    src/overlaycontroller.cpp  src/overlaycontroller.h
    src/recentmodel.cpp        src/recentmodel.h
    src/selection.cpp          src/selection.h
    src/settings.cpp           src/settings.h
    src/surfaces.cpp           src/surfaces.h
    src/toastcontroller.cpp    src/toastcontroller.h
    src/tray.cpp               src/tray.h
    src/wheelgeometry.cpp      src/wheelgeometry.h
    src/wheelmodel.cpp         src/wheelmodel.h
    src/wheelquery.cpp         src/wheelquery.h
)
target_include_directories(nashi_ui PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src)
target_link_libraries(nashi_ui PUBLIC
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::Widgets Qt6::DBus)
if(NASHI_WITH_LAYERSHELL)
    target_link_libraries(nashi_ui PUBLIC LayerShellQt::Interface)
    target_compile_definitions(nashi_ui PUBLIC NASHI_HAVE_LAYERSHELL=1)
endif()

set(NASHI_QML_FILES
    qml/HomeWindow.qml
    qml/OverlayWindow.qml
    qml/ToastWindow.qml
    qml/Wheel.qml
    qml/WheelSlice.qml
    qml/JobCard.qml
    qml/ItemGrid.qml
    qml/DropPage.qml
    qml/RecentPage.qml
    qml/DepsPage.qml
    qml/SettingsPage.qml
)

qt_add_qml_module(nashi_ui
    URI NashiApp
    VERSION 1.0
    QML_FILES ${NASHI_QML_FILES}
    RESOURCES resources/nashi.svg
    RESOURCE_PREFIX /qt/qml
    OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/NashiApp
)

# --- Application -------------------------------------------------------------
qt_add_executable(nashi src/main.cpp)
target_link_libraries(nashi PRIVATE nashi_ui nashi_uiplugin)

# Translations are embedded in nashi_ui (:/i18n/nashi_tr.qm) so the tests see them too.
qt_add_translations(nashi_ui
    TS_FILES i18n/nashi_tr.ts
    SOURCE_TARGETS nashi_ui nashi
    RESOURCE_PREFIX /i18n
    LUPDATE_OPTIONS -no-obsolete -locations none
)

install(TARGETS nashi RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})

# --- Tests -------------------------------------------------------------------
include(CTest)
if(BUILD_TESTING)
    add_subdirectory(tests)
endif()
