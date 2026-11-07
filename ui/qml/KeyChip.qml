import QtQuick
import NashiApp

// "Shift  Convert" chord hint: a small outlined card, as on upstream Windows.
Rectangle {
    id: chip
    property string keyText: ""
    property string label: ""

    implicitWidth: row.implicitWidth + 18
    implicitHeight: row.implicitHeight + 10
    radius: 9
    color: Theme.surface
    border.width: 1
    border.color: Theme.line

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: chip.keyText
            font.pixelSize: Theme.sizeSmall
            font.weight: Font.Medium
            color: Theme.secondaryText
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: chip.label
            font.pixelSize: Theme.sizeSmall
            color: Theme.secondaryText
        }
    }
}
