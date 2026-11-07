pragma Singleton

import QtQuick
import NashiApp

// Shared visual language, after upstream Nashi's DesignSystem.swift: warm
// paper canvas, forest green and pear gold, rounded cards. Every colour has a
// light and a dark value; `dark` follows Settings → Appearance or the system.
QtObject {
    id: theme

    readonly property string appearance: App.settings.appearance
    readonly property bool dark: appearance === "dark"
                                 || (appearance === "system" && Qt.styleHints.colorScheme === Qt.ColorScheme.Dark)
    readonly property bool motion: !App.settings.reduceMotion

    function pick(light, darkValue) { return theme.dark ? darkValue : light }

    // Surfaces
    readonly property color canvas: pick("#f9f6ed", "#111914")
    readonly property color surface: pick("#fffdf7", "#1c271f")
    readonly property color inset: pick("#f1ede0", "#151f18")
    readonly property color border: pick(Qt.rgba(0, 0, 0, 0.075), Qt.rgba(1, 1, 1, 0.08))
    // Firmer outline for small chips and badges (upstream Windows "Line").
    readonly property color line: pick(Qt.rgba(0, 0, 0, 0.13), "#344237")
    readonly property color hover: pick(Qt.rgba(0, 0, 0, 0.035), Qt.rgba(1, 1, 1, 0.05))

    // Text
    readonly property color text: pick("#1d2117", "#eef0e8")
    readonly property color secondaryText: pick(Qt.rgba(0.11, 0.13, 0.09, 0.62), Qt.rgba(0.93, 0.94, 0.91, 0.62))
    readonly property color tertiaryText: pick(Qt.rgba(0.11, 0.13, 0.09, 0.4), Qt.rgba(0.93, 0.94, 0.91, 0.4))

    // Brand
    readonly property color gold: "#e3b859"
    readonly property color goldEnd: "#f5d17d"
    readonly property color goldInk: pick("#704f17", "#e8c475")
    readonly property color ink: "#334024"
    readonly property color buttonInk: "#2b361c"
    readonly property color forest: pick("#30543b", "#b3d19e")
    readonly property color sand: pick("#f6e7be", "#3c3421")
    readonly property color sage: pick("#e0ebd2", "#243828")
    readonly property color sky: pick("#5778ab", "#9db7df")
    readonly property color danger: pick("#c0392b", "#ff8a7a")
    readonly property color success: pick("#2f7d4a", "#8fd19e")

    // Shape
    readonly property int radiusCard: 22
    readonly property int radiusZone: 26
    readonly property int radiusRow: 16
    readonly property int radiusButton: 13
    readonly property int radiusCompact: 10

    // Type
    readonly property int sizeHero: 30
    readonly property int sizeTitle: 23
    readonly property int sizeHeading: 16
    readonly property int sizeBody: 13
    readonly property int sizeSmall: 11
    readonly property int sizeTiny: 10

    // Motion
    readonly property int quick: motion ? 140 : 0
    readonly property int normal: motion ? 240 : 0
}
