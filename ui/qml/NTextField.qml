import QtQuick
import QtQuick.Templates as T
import NashiApp

// Text field after the upstream Windows style: inset fill, line border that
// turns gold while typing.
T.TextField {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentWidth + leftPadding + rightPadding,
                            placeholder.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding)
    leftPadding: 11
    rightPadding: 11
    topPadding: 8
    bottomPadding: 8
    selectByMouse: true
    color: Theme.text
    selectionColor: Theme.gold
    selectedTextColor: Theme.buttonInk
    placeholderTextColor: Theme.tertiaryText
    font.pixelSize: Theme.sizeBody
    verticalAlignment: TextInput.AlignVCenter
    opacity: enabled ? 1 : 0.5

    Text {
        id: placeholder
        x: control.leftPadding
        y: control.topPadding
        width: control.width - (control.leftPadding + control.rightPadding)
        height: control.height - (control.topPadding + control.bottomPadding)
        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: control.verticalAlignment
        elide: Text.ElideRight
        visible: !control.length && !control.preeditText
    }

    background: Rectangle {
        implicitWidth: 160
        radius: Theme.radiusCompact
        color: Theme.inset
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Theme.gold : Theme.line
    }
}
