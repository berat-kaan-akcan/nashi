#include "setupcheck.h"

#include <QSettings>

#include <grp.h>
#include <pwd.h>
#include <sys/types.h>
#include <unistd.h>

#include <vector>

namespace {

QList<unsigned> processGroups()
{
    QList<unsigned> out{unsigned(getegid())};
    const int n = getgroups(0, nullptr);
    if (n > 0) {
        std::vector<gid_t> gids(static_cast<size_t>(n));
        const int got = getgroups(n, gids.data());
        for (int i = 0; i < got; ++i)
            out.append(unsigned(gids[size_t(i)]));
    }
    return out;
}

// Groups the account database (NSS) lists for `user`, including `primary`.
QList<unsigned> accountGroups(const QByteArray &user, gid_t primary)
{
    QList<unsigned> out;
    if (user.isEmpty())
        return out;
    int n = 64;
    std::vector<gid_t> gids(static_cast<size_t>(n));
    if (getgrouplist(user.constData(), primary, gids.data(), &n) == -1) {
        // `n` now holds the required size.
        if (n <= 0)
            return out;
        gids.resize(size_t(n));
        if (getgrouplist(user.constData(), primary, gids.data(), &n) == -1)
            return out;
    }
    for (int i = 0; i < n; ++i)
        out.append(unsigned(gids[size_t(i)]));
    return out;
}

} // namespace

SetupCheck::SetupCheck(QSettings *store, QObject *parent)
    : QObject(parent)
    , m_store(store)
{
    refresh();
}

bool SetupCheck::welcomeDone() const
{
    return m_store && m_store->value(kWelcomeKey, false).toBool();
}

void SetupCheck::setWelcomeDone(bool done)
{
    if (!m_store || welcomeDone() == done)
        return;
    m_store->setValue(kWelcomeKey, done);
    m_store->sync();
    Q_EMIT welcomeDoneChanged();
}

QString SetupCheck::groupCommand() const
{
    return QStringLiteral("sudo usermod -aG input $USER");
}

QString SetupCheck::serviceCommand() const
{
    return QStringLiteral("systemctl --user enable --now nashi-inputd.service");
}

SetupCheck::GroupState SetupCheck::evaluate(bool groupExists, unsigned inputGid, const QList<unsigned> &processGids,
                                            const QList<unsigned> &accountGids)
{
    if (!groupExists)
        return NoGroup;
    if (processGids.contains(inputGid))
        return InGroup;
    if (accountGids.contains(inputGid))
        return NeedsRelogin;
    return NotInGroup;
}

void SetupCheck::refresh()
{
    // Reentrant lookups; buffers sized generously for large group lists.
    std::vector<char> buf(1 << 16);

    QByteArray user;
    gid_t primary = getgid();
    struct passwd pw{};
    struct passwd *pwResult = nullptr;
    if (getpwuid_r(getuid(), &pw, buf.data(), buf.size(), &pwResult) == 0 && pwResult) {
        user = QByteArray(pw.pw_name);
        primary = pw.pw_gid;
    }

    bool exists = false;
    gid_t inputGid = 0;
    struct group gr{};
    struct group *grResult = nullptr;
    if (getgrnam_r(kGroupName.toLatin1().constData(), &gr, buf.data(), buf.size(), &grResult) == 0 && grResult) {
        exists = true;
        inputGid = gr.gr_gid;
    }

    const GroupState state = evaluate(exists, unsigned(inputGid), processGroups(), accountGroups(user, primary));
    const QString name = QString::fromLocal8Bit(user);
    if (state == m_groupState && name == m_userName)
        return;
    m_groupState = state;
    m_userName = name;
    Q_EMIT changed();
}
