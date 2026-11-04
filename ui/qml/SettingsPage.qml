import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

Page {
    id: page
    background: null
    padding: 0

    readonly property Settings settings: App.settings
    readonly property InputdClient inputd: App.inputd

    readonly property var chordModes: [
        { mode: "convert", title: qsTr("Convert") },
        { mode: "tools", title: qsTr("Tools") },
        { mode: "actions", title: qsTr("Actions") }
    ]
    readonly property var modifierKeys: ["shift", "ctrl", "alt", "meta"]

    function keyLabel(key) {
        if (key === "shift") return "⇧ Shift"
        if (key === "ctrl") return "Ctrl"
        if (key === "alt") return "Alt"
        if (key === "meta") return "Meta"
        return "—"
    }

    function chordKey(mode) {
        if (mode === "tools") return inputd.toolsKey
        if (mode === "actions") return inputd.actionsKey
        return inputd.convertKey
    }

    // Gives `mode` the modifier `key`. A wheel that had `key` before takes
    // over the old key of `mode`, so the three keys always stay distinct.
    function assignChord(mode, key) {
        const keys = { convert: inputd.convertKey, tools: inputd.toolsKey, actions: inputd.actionsKey }
        const old = keys[mode]
        for (const m in keys) {
            if (m !== mode && keys[m] === key)
                keys[m] = old
        }
        keys[mode] = key
        for (const m in keys) {
            if (modifierKeys.indexOf(keys[m]) < 0) {
                const used = Object.values(keys)
                keys[m] = modifierKeys.find(k => used.indexOf(k) < 0)
            }
        }
        inputd.setChords(keys.convert, keys.tools, keys.actions)
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Output folder")
        onAccepted: page.settings.customOutputDir = page.settings.urlToLocalPath(selectedFolder)
    }

    component Hint: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeSmall
        color: Theme.secondaryText
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            x: 28
            width: scroll.availableWidth - 56
            spacing: 14

            PageHeading {
                Layout.fillWidth: true
                Layout.topMargin: 8
                Layout.bottomMargin: 6
                title: qsTr("Settings")
                subtitle: qsTr("Make Nashi feel like yours.")
            }

            SectionBox {
                title: qsTr("Startup")
                Switch {
                    id: loginSwitch
                    text: qsTr("Start Nashi when I log in")
                    checked: page.settings.startAtLogin
                    onToggled: {
                        page.settings.startAtLogin = checked
                        // Snap back to the real state if writing failed.
                        checked = Qt.binding(() => page.settings.startAtLogin)
                    }
                }
                Hint {
                    text: qsTr("Nashi waits in the system tray so drag chords work right after you log in.")
                }
                Hint {
                    visible: text.length > 0
                    color: Theme.danger
                    text: page.settings.startAtLoginError
                }
                Switch {
                    objectName: "showShelfOnStartupSwitch"
                    text: qsTr("Show Float Shelf on startup")
                    checked: page.settings.showShelfOnStartup
                    onToggled: page.settings.showShelfOnStartup = checked
                }
            }

            SectionBox {
                title: qsTr("Appearance")
                RowLayout {
                    spacing: 6
                    Repeater {
                        model: [
                            { value: "system", text: qsTr("System") },
                            { value: "light", text: qsTr("Light") },
                            { value: "dark", text: qsTr("Dark") }
                        ]
                        delegate: NButton {
                            required property var modelData
                            compact: true
                            kind: page.settings.appearance === modelData.value ? "primary" : "secondary"
                            text: modelData.text
                            onClicked: page.settings.appearance = modelData.value
                        }
                    }
                }
                Switch {
                    text: qsTr("Reduce animations")
                    checked: page.settings.reduceMotion
                    onToggled: page.settings.reduceMotion = checked
                }
            }

            SectionBox {
                title: qsTr("Language")
                ComboBox {
                    id: lang
                    Layout.preferredWidth: 280
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        { value: "auto", text: qsTr("System default") },
                        { value: "en", text: "English" },
                        { value: "tr", text: "Türkçe" }
                    ]
                    currentIndex: Math.max(0, indexOfValue(page.settings.language))
                    onActivated: page.settings.language = currentValue
                }
            }

            SectionBox {
                title: qsTr("Output")
                RadioButton {
                    text: qsTr("Save next to the original file")
                    checked: !page.settings.useCustomOutputDir
                    onToggled: if (checked) page.settings.useCustomOutputDir = false
                }
                RadioButton {
                    id: customRadio
                    text: qsTr("Save into a folder:")
                    checked: page.settings.useCustomOutputDir
                    onToggled: if (checked) page.settings.useCustomOutputDir = true
                }
                RowLayout {
                    Layout.fillWidth: true
                    enabled: customRadio.checked
                    opacity: enabled ? 1 : 0.5
                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Choose a folder")
                        text: page.settings.customOutputDir
                        onEditingFinished: page.settings.customOutputDir = text
                    }
                    NButton {
                        compact: true
                        iconName: "folder-open"
                        text: qsTr("Browse…")
                        onClicked: folderDialog.open()
                    }
                }
                Hint {
                    text: qsTr("Existing files are never overwritten: a number is added to the name instead.")
                }
            }

            SectionBox {
                title: qsTr("Wheel")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: qsTr("Size")
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                    }
                    Slider {
                        id: scaleSlider
                        Layout.fillWidth: true
                        from: page.settings.minWheelScale
                        to: page.settings.maxWheelScale
                        stepSize: 0.05
                        snapMode: Slider.SnapAlways
                        value: page.settings.wheelScale
                        onMoved: page.settings.wheelScale = value
                        Accessible.name: qsTr("Wheel size")
                    }
                    Text {
                        Layout.preferredWidth: 44
                        horizontalAlignment: Text.AlignRight
                        text: qsTr("%1%").arg(Math.round(scaleSlider.value * 100))
                        font.pixelSize: Theme.sizeBody
                        color: Theme.secondaryText
                    }
                    NButton {
                        compact: true
                        kind: "quiet"
                        text: qsTr("Reset")
                        enabled: page.settings.wheelScale !== 1.0
                        onClicked: page.settings.wheelScale = 1.0
                    }
                }
                Hint {
                    text: qsTr("Makes the drag wheel smaller or larger. Slices, labels and drop targets scale together.")
                }
            }

            SectionBox {
                title: qsTr("Notifications")
                Switch {
                    text: qsTr("Desktop notifications when a job finishes")
                    checked: page.settings.notifications
                    onToggled: page.settings.notifications = checked
                }
                Hint {
                    text: qsTr("Sent when the progress popup is closed or the job took longer than 10 seconds.")
                }
            }

            SectionBox {
                title: qsTr("Drag chords")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Rectangle {
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        radius: 4
                        color: App.inputd.available ? Theme.success : Theme.danger
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                        text: App.inputd.available
                              ? qsTr("nashi-inputd is running (version %1).").arg(App.inputd.version.length > 0 ? App.inputd.version : "?")
                              : qsTr("nashi-inputd is not running. Enable it with: systemctl --user enable --now nashi-inputd")
                    }
                }
                Switch {
                    text: qsTr("Show the wheel when I hold a chord key while dragging")
                    enabled: App.inputd.available
                    checked: App.inputd.available && App.inputd.enabled
                    onToggled: App.inputd.setEnabled(checked)
                }
                Repeater {
                    model: page.chordModes
                    delegate: RowLayout {
                        id: chordRow
                        required property var modelData
                        readonly property string current: page.chordKey(modelData.mode)
                        Layout.fillWidth: true
                        spacing: 6
                        KeyChip {
                            Layout.preferredWidth: 150
                            keyText: page.keyLabel(chordRow.current)
                            label: chordRow.modelData.title
                        }
                        Item { Layout.fillWidth: true }
                        Repeater {
                            model: page.modifierKeys
                            delegate: NButton {
                                required property string modelData
                                compact: true
                                kind: chordRow.current === modelData ? "primary" : "secondary"
                                text: page.keyLabel(modelData)
                                Accessible.name: qsTr("Open the %1 wheel with %2").arg(chordRow.modelData.title).arg(page.keyLabel(modelData))
                                onClicked: if (chordRow.current !== modelData) page.assignChord(chordRow.modelData.mode, modelData)
                            }
                        }
                    }
                }
                Hint {
                    text: qsTr("Each wheel has its own key; picking a key that is in use swaps the two.")
                }
                Hint {
                    visible: page.inputd.convertKey === "meta" || page.inputd.toolsKey === "meta" || page.inputd.actionsKey === "meta"
                    text: qsTr("KDE Plasma also uses Meta + drag to move windows; Nashi only sees the drag if the window manager lets it through.")
                }
                Hint {
                    visible: text.length > 0
                    color: Theme.danger
                    text: page.inputd.configError
                }
                Hint {
                    text: App.layerShell ? qsTr("Overlay: Wayland layer-shell.")
                                         : qsTr("Overlay: full-screen window fallback (no layer-shell).")
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                Layout.bottomMargin: 16
                spacing: 8
                NashiMark { size: 20; happy: true }
                Text {
                    Layout.fillWidth: true
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.tertiaryText
                    text: qsTr("Nashi %1 · nashi-cli: %2").arg(App.version).arg(App.cliPath.length > 0 ? App.cliPath : qsTr("not found"))
                    elide: Text.ElideMiddle
                }
            }
        }
    }
}
