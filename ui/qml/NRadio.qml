import QtQuick
import QtQuick.Layouts
import QtQuick.Templates as T
import NashiApp

// Radio button in Nashi colours: gold ring and dot when chosen.
T.RadioButton {
    id: control

    // A settings row: long labels wrap inside the card.
    Layout.fillWidth: true

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)
    padding: 4
    spacing: 10
    hoverEnabled: true
    opacity: enabled ? 1 : 0.4

    indicator: Rectangle {
        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitWidth: 20
        implicitHeight: 20
        radius: 10
        color: Theme.surface
        border.width: control.checked ? 2 : 1
        border.color: control.checked || control.hovered ? Theme.gold : Theme.line

        Rectangle {
            anchors.centerIn: parent
            width: 10
            height: 10
            radius: 5
            color: Theme.gold
            visible: control.checked
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -3
            radius: width / 2
            color: "transparent"
            border.width: 2
            border.color: Theme.goldInk
            visible: control.visualFocus
        }
    }

    contentItem: Text {
        leftPadding: control.indicator.width + control.spacing
        verticalAlignment: Text.AlignVCenter
        text: control.text
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeBody
        color: Theme.text
    }
}
