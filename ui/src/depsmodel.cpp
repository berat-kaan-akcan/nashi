#include "depsmodel.h"
#include "wheelquery.h"

#include <QJsonArray>
#include <QPointer>

DepsModel::DepsModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

QList<DepsModel::Tool> DepsModel::parse(const QJsonObject &deps)
{
    QList<Tool> tools;
    for (const QJsonValue &v : deps.value(QStringLiteral("tools")).toArray()) {
        const QJsonObject o = v.toObject();
        Tool t;
        t.id = o.value(QStringLiteral("id")).toString();
        if (t.id.isEmpty())
            continue;
        t.available = o.value(QStringLiteral("available")).toBool();
        t.version = o.value(QStringLiteral("version")).toString();
        t.path = o.value(QStringLiteral("path")).toString();
        t.package = o.value(QStringLiteral("package")).toString();
        t.optional = o.value(QStringLiteral("optional")).toBool();
        for (const QJsonValue &u : o.value(QStringLiteral("used_for")).toArray())
            t.usedFor.append(u.toString());
        tools.append(t);
    }
    // Missing first, then required before optional, then by name.
    std::stable_sort(tools.begin(), tools.end(), [](const Tool &a, const Tool &b) {
        if (a.available != b.available)
            return !a.available;
        if (a.optional != b.optional)
            return !a.optional;
        return a.id < b.id;
    });
    return tools;
}

QString DepsModel::pacmanCommand(const QStringList &packages)
{
    if (packages.isEmpty())
        return {};
    return QStringLiteral("sudo pacman -S --needed ") + packages.join(QLatin1Char(' '));
}

void DepsModel::refresh()
{
    if (m_loading)
        return;
    m_loading = true;
    Q_EMIT loadingChanged();
    QPointer<DepsModel> self(this);
    CliCall::run(this, m_program, {QStringLiteral("deps"), QStringLiteral("--json")},
                 [self](bool ok, const QJsonObject &obj, const QString &err) {
                     if (!self)
                         return;
                     self->m_loading = false;
                     if (ok) {
                         self->m_error.clear();
                         self->setData(obj);
                     } else {
                         self->m_error = err;
                         Q_EMIT self->changed();
                     }
                     Q_EMIT self->loadingChanged();
                 });
}

void DepsModel::setData(const QJsonObject &deps)
{
    beginResetModel();
    m_tools = parse(deps);
    endResetModel();
    Q_EMIT changed();
}

int DepsModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_tools.size());
}

QVariant DepsModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_tools.size())
        return {};
    const Tool &t = m_tools.at(index.row());
    switch (role) {
    case Qt::DisplayRole:
    case IdRole:
        return t.id;
    case AvailableRole:
        return t.available;
    case VersionRole:
        return t.version;
    case PathRole:
        return t.path;
    case PackageRole:
        return t.package;
    case OptionalRole:
        return t.optional;
    case UsedForRole:
        return t.usedFor.join(QStringLiteral(", "));
    default:
        return {};
    }
}

QHash<int, QByteArray> DepsModel::roleNames() const
{
    return {{IdRole, "toolId"},     {AvailableRole, "available"}, {VersionRole, "version"}, {PathRole, "path"},
            {PackageRole, "packageName"}, {OptionalRole, "optional"},   {UsedForRole, "usedFor"}};
}

int DepsModel::missingCount() const
{
    return int(std::count_if(m_tools.cbegin(), m_tools.cend(), [](const Tool &t) { return !t.available; }));
}

QStringList DepsModel::missingPackages(bool includeOptional) const
{
    QStringList pkgs;
    for (const Tool &t : m_tools) {
        if (t.available || t.package.isEmpty() || (t.optional && !includeOptional))
            continue;
        if (!pkgs.contains(t.package))
            pkgs.append(t.package);
    }
    return pkgs;
}
