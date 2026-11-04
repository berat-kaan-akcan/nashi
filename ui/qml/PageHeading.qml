import QtQuick
import QtQuick.Layouts
import NashiApp

// Eyebrow + large title + subtitle.
ColumnLayout {
    property string eyebrow: ""
    property string title: ""
    property string subtitle: ""
    spacing: 6

    Text {
        visible: text.length > 0
        text: parent.eyebrow.toUpperCase()
        font.pixelSize: Theme.sizeTiny
        font.weight: Font.Bold
        font.letterSpacing: 1.4
        color: Theme.goldInk
    }
    Text {
        Layout.fillWidth: true
        text: parent.title
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeHero
        font.weight: Font.Bold
        font.letterSpacing: -0.8
        color: Theme.text
    }
    Text {
        Layout.fillWidth: true
        visible: text.length > 0
        text: parent.subtitle
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeBody
        color: Theme.secondaryText
    }
}
