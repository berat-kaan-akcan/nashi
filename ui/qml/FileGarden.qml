import QtQuick
import QtQuick.Shapes
import NashiApp

// The drop zone illustration, after upstream HomeView.FileGarden (macOS) and
// FileGarden.cs (Windows): a sand circle, a PDF and a PNG card, and the pear.
Item {
    id: garden
    property bool targeted: false
    property bool selected: false

    implicitWidth: 166
    implicitHeight: 150
    Accessible.ignored: true

    Rectangle {
        anchors.centerIn: parent
        width: 144
        height: 144
        radius: 72
        color: Theme.sand
    }

    component DocCard: Rectangle {
        id: card
        property string label: ""
        property color tint: Theme.forest
        property string glyph: ""
        property real dx: 0
        property real dy: 0

        width: 77
        height: 103
        radius: 15
        x: (garden.width - width) / 2 + dx
        y: (garden.height - height) / 2 + dy
        color: Theme.surface
        border.width: 1
        border.color: Qt.rgba(tint.r, tint.g, tint.b, 0.16)
        antialiasing: true
        Behavior on rotation { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack } }

        Column {
            anchors.fill: parent
            anchors.margins: 13
            spacing: 8
            // Drawn rather than taken from the icon theme so both cards always match.
            Shape {
                width: 32
                height: 30
                ShapePath {
                    strokeColor: card.tint
                    strokeWidth: 2
                    fillColor: "transparent"
                    joinStyle: ShapePath.RoundJoin
                    capStyle: ShapePath.RoundCap
                    PathSvg { path: card.glyph }
                }
            }
            Text {
                text: card.label
                font.pixelSize: 10
                font.weight: Font.Bold
                color: card.tint
            }
            Rectangle {
                width: 37
                height: 4
                radius: 2
                color: Qt.rgba(card.tint.r, card.tint.g, card.tint.b, 0.18)
            }
        }
    }

    DocCard {
        label: "PDF"
        tint: Theme.forest
        glyph: "M5,1 L22,1 L29,8 L29,29 L5,29 Z M22,1 L22,8 L29,8 M10,14 L24,14 M10,19 L24,19 M10,24 L20,24"
        dx: -34
        dy: 12
        rotation: -16
    }
    DocCard {
        label: "PNG"
        tint: Theme.dark ? Theme.gold : Theme.goldInk
        glyph: "M2,4 L30,4 L30,27 L2,27 Z M3,25 L12,14 L18,20 L23,14 L30,23 M11,10 A2,2 0 1 1 7,10 A2,2 0 1 1 11,10"
        dx: 31
        dy: -2
        rotation: garden.targeted && Theme.motion ? 13 : 7
    }

    NashiMark {
        size: 65
        happy: garden.selected || garden.targeted
        x: (garden.width - width) / 2
        y: (garden.height - height) / 2 + 42
        rotation: garden.targeted && Theme.motion ? -8 : 0
        Behavior on rotation { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack } }
    }
}
