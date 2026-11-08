import QtQuick
import QtQuick.Layouts
import QtQuick.Templates as T
import NashiApp

// On/off switch in Nashi colours: a gold track when on, the inset colour
// when off. Built on the templates so the desktop style cannot recolour it.
T.Switch {
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
        implicitWidth: 38
        implicitHeight: 22
        radius: height / 2
        color: control.checked ? Theme.gold : Theme.inset
        border.width: control.checked ? 0 : 1
        border.color: control.hovered ? Theme.gold : Theme.line
        Behavior on color { ColorAnimation { duration: Theme.quick } }

        Rectangle {
            width: 16
            height: 16
            radius: 8
            y: (parent.height - height) / 2
            x: control.checked ? parent.width - width - 3 : 3
            color: control.checked ? "#fffdf7" : Theme.surface
            border.width: 1
            border.color: control.checked ? Qt.rgba(0, 0, 0, 0.08) : Theme.line
            Behavior on x { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -3
            radius: height / 2
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
