import QtQuick
import NashiApp

// "⇧ Convert" style chord hint.
Rectangle {
    id: chip
    property string keyText: ""
    property string label: ""

    implicitWidth: row.implicitWidth + 18
    implicitHeight: row.implicitHeight + 12
    radius: 8
    color: Qt.rgba(Theme.sage.r, Theme.sage.g, Theme.sage.b, 0.75)

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5
        Text {
            text: chip.keyText
            font.pixelSize: Theme.sizeSmall + 1
            font.weight: Font.DemiBold
            color: Theme.forest
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: chip.label
            font.pixelSize: Theme.sizeTiny
            font.weight: Font.Medium
            color: Theme.forest
        }
    }
}
