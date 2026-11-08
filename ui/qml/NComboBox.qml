import QtQuick
import QtQuick.Controls
import QtQuick.Templates as T
import NashiApp

// Drop-down after the upstream Windows style: surface box with a chevron,
// a rounded list whose current and hovered rows use the sage tint.
T.ComboBox {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)
    leftPadding: 12
    rightPadding: 32
    topPadding: 8
    bottomPadding: 8
    hoverEnabled: true
    opacity: enabled ? 1 : 0.4

    delegate: T.ItemDelegate {
        width: ListView.view.width
        implicitHeight: label.implicitHeight + topPadding + bottomPadding
        padding: 8
        leftPadding: 10
        highlighted: control.highlightedIndex === index
        hoverEnabled: true
        text: control.textRole
              ? (Array.isArray(control.model) ? modelData[control.textRole] : model[control.textRole])
              : modelData
        contentItem: Text {
            id: label
            text: parent.text
            font.pixelSize: Theme.sizeBody
            font.weight: control.currentIndex === index ? Font.DemiBold : Font.Normal
            color: Theme.text
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 6
            color: parent.highlighted || parent.hovered || control.currentIndex === index
                   ? Theme.sage : "transparent"
        }
    }

    indicator: Text {
        x: control.width - width - 12
        y: (control.height - height) / 2
        text: "⌄"
        font.pixelSize: Theme.sizeBody + 2
        color: Theme.text
    }

    contentItem: Text {
        text: control.displayText
        font.pixelSize: Theme.sizeBody
        color: Theme.text
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        implicitWidth: 140
        radius: Theme.radiusCompact
        color: control.hovered ? Qt.tint(Theme.surface, Theme.hover) : Theme.surface
        border.width: control.visualFocus || control.popup.visible ? 2 : 1
        border.color: control.visualFocus || control.popup.visible ? Theme.gold : Theme.line
    }

    popup: T.Popup {
        y: control.height + 4
        width: Math.max(control.width, 160)
        implicitHeight: Math.min(contentItem.implicitHeight + topPadding + bottomPadding, 320)
        padding: 5

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: control.delegateModel
            currentIndex: control.highlightedIndex
            ScrollIndicator.vertical: ScrollIndicator {}
        }

        background: Rectangle {
            radius: Theme.radiusCompact
            color: Theme.surface
            border.width: 1
            border.color: Theme.line
        }
    }
}
