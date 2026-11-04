import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// One of the three Home tiles (Convert / Fine-tune / Move & share).
AbstractButton {
    id: tile
    property string subtitle: ""
    property string iconName: ""
    property color tint: Theme.gold
    property bool selected: false

    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.45
    Accessible.role: Accessible.Button
    Accessible.name: text

    background: Rectangle {
        radius: Theme.radiusCard
        color: Theme.surface
        border.width: tile.selected ? 2 : 1
        border.color: tile.selected ? tile.tint : Theme.border
        Behavior on border.color { ColorAnimation { duration: Theme.quick } }
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: tile.hovered ? Theme.hover : "transparent"
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -3
            radius: parent.radius + 3
            color: "transparent"
            border.width: 2
            border.color: Theme.goldInk
            visible: tile.visualFocus
        }
    }

    contentItem: ColumnLayout {
        spacing: 4
        Rectangle {
            Layout.preferredWidth: 34
            Layout.preferredHeight: 34
            Layout.bottomMargin: 4
            radius: 10
            color: Qt.rgba(tile.tint.r, tile.tint.g, tile.tint.b, 0.16)
            Image {
                anchors.centerIn: parent
                source: tile.iconName.length > 0 ? "image://icon/" + tile.iconName : ""
                sourceSize: Qt.size(40, 40)
                width: 20
                height: 20
            }
        }
        Text {
            Layout.fillWidth: true
            text: tile.text
            elide: Text.ElideRight
            font.pixelSize: Theme.sizeBody + 1
            font.weight: Font.Bold
            color: Theme.text
        }
        Text {
            Layout.fillWidth: true
            text: tile.subtitle
            elide: Text.ElideRight
            font.pixelSize: Theme.sizeSmall
            color: Theme.secondaryText
        }
    }
    leftPadding: 16
    rightPadding: 16
    topPadding: 14
    bottomPadding: 14
}
