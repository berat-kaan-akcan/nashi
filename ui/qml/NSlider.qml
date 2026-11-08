import QtQuick
import QtQuick.Templates as T
import NashiApp

// Horizontal slider in Nashi colours: gold fill, surface handle.
T.Slider {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitHandleWidth + leftPadding + rightPadding, 160)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitHandleHeight + topPadding + bottomPadding)
    padding: 4
    hoverEnabled: true
    opacity: enabled ? 1 : 0.4

    background: Rectangle {
        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitHeight: 6
        width: control.availableWidth
        height: implicitHeight
        radius: 3
        color: Theme.inset
        border.width: 1
        border.color: Theme.line

        Rectangle {
            width: control.visualPosition * parent.width
            height: parent.height
            radius: 3
            color: Theme.gold
        }
    }

    handle: Rectangle {
        x: control.leftPadding + control.visualPosition * (control.availableWidth - width)
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitWidth: 20
        implicitHeight: 20
        radius: 10
        color: Theme.surface
        border.width: control.pressed || control.visualFocus ? 2 : 1
        border.color: control.pressed || control.hovered || control.visualFocus ? Theme.gold : Theme.line
    }
}
