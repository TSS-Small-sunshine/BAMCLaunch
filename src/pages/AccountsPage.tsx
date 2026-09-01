import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert as ChakraAlert,
  AlertIcon,
  Avatar,
  Badge,
  Box,
  Button,
  Flex,
  HStack,
  Heading,
  Input,
  Text,
  VStack,
} from '@chakra-ui/react';
import { AddIcon, CheckCircleIcon, DeleteIcon, RepeatIcon } from '@chakra-ui/icons';
import { addOfflineAccount, listAccounts, removeAccount, setActiveAccount } from '../lib/tauri';
import type { Account, OfflineAccount } from '../types/account';

/** M3 / L1:账户管理页 —— 离线账户的增删 + 切换当前账户 */
export default function AccountsPage() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newUsername, setNewUsername] = useState('');
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [switching, setSwitching] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listAccounts();
      setAccounts(list);
    } catch (e) {
      setError(`加载账户失败: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleAdd = async () => {
    const username = newUsername.trim();
    if (!username) return;
    setAdding(true);
    setError(null);
    try {
      await addOfflineAccount(username);
      setNewUsername('');
      await reload();
    } catch (e) {
      setError(`添加失败: ${String(e)}`);
    } finally {
      setAdding(false);
    }
  };

  const handleRemove = async (id: string) => {
    setRemoving(id);
    setError(null);
    try {
      await removeAccount(id);
      if (activeId === id) setActiveId(null);
      await reload();
    } catch (e) {
      setError(`删除失败: ${String(e)}`);
    } finally {
      setRemoving(null);
    }
  };

  const handleActivate = async (id: string) => {
    setSwitching(id);
    setError(null);
    try {
      await setActiveAccount(id);
      setActiveId(id);
    } catch (e) {
      setError(`切换失败: ${String(e)}`);
    } finally {
      setSwitching(null);
    }
  };

  const offlineAccounts = useMemo(
    () => accounts.filter((a): a is OfflineAccount => a.type === 'offline'),
    [accounts]
  );

  return (
    <Box maxW="720px" mx="auto">
      <Flex align="baseline" justify="space-between" mb={6}>
        <Box>
          <Heading size="lg" color="gray.800" mb={1}>
            账户管理
          </Heading>
          <Text fontSize="sm" color="gray.500">
            离线模式(无正版验证)· 微软账户登录将在 M3 L2 实装
          </Text>
        </Box>
        <Button
          size="sm"
          variant="ghost"
          leftIcon={<RepeatIcon />}
          onClick={() => void reload()}
          isLoading={loading}
        >
          刷新
        </Button>
      </Flex>

      {error && (
        <ChakraAlert status="error" borderRadius="card" mb={4} bg="red.50">
          <AlertIcon />
          {error}
        </ChakraAlert>
      )}

      {/* 添加账户输入条 */}
      <Box
        bg="white"
        borderRadius="card"
        border="1px solid"
        borderColor="brand.100"
        boxShadow="card"
        p={4}
        mb={4}
      >
        <Heading size="xs" color="gray.600" mb={2} fontWeight="700">
          添加离线账户
        </Heading>
        <Text fontSize="xs" color="gray.500" mb={3}>
          用户名 3-16 字符,只允许字母、数字和下划线(Mojang 离线模式规则)
        </Text>
        <HStack spacing={2}>
          <Input
            value={newUsername}
            onChange={(e) => setNewUsername(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !adding) void handleAdd();
            }}
            placeholder="例如 Steve"
            fontFamily="mono"
            fontSize="sm"
            maxLength={16}
          />
          <Button
            size="sm"
            colorScheme="brand"
            leftIcon={<AddIcon />}
            onClick={() => void handleAdd()}
            isLoading={adding}
            isDisabled={newUsername.trim().length < 3}
          >
            添加
          </Button>
        </HStack>
      </Box>

      {/* 账户列表 */}
      {loading && offlineAccounts.length === 0 ? (
        <Flex justify="center" py={10}>
          <Text color="gray.500" fontSize="sm">
            加载中...
          </Text>
        </Flex>
      ) : offlineAccounts.length === 0 ? (
        <ChakraAlert status="info" borderRadius="card" bg="blue.50">
          <AlertIcon />
          <Box>
            <Text fontWeight="700" color="blue.700" fontSize="sm">
              还没有任何账户
            </Text>
            <Text fontSize="xs" color="blue.600" mt={1}>
              在上方输入用户名,点「添加」创建一个离线账户
            </Text>
          </Box>
        </ChakraAlert>
      ) : (
        <VStack spacing={2.5} align="stretch">
          {offlineAccounts.map((acc) => (
            <AccountRow
              key={acc.id}
              account={acc}
              isActive={acc.id === activeId}
              onActivate={() => void handleActivate(acc.id)}
              activating={switching === acc.id}
              onRemove={() => void handleRemove(acc.id)}
              removing={removing === acc.id}
            />
          ))}
        </VStack>
      )}
    </Box>
  );
}

function AccountRow({
  account,
  isActive,
  onActivate,
  activating,
  onRemove,
  removing,
}: {
  account: OfflineAccount;
  isActive: boolean;
  onActivate: () => void;
  activating: boolean;
  onRemove: () => void;
  removing: boolean;
}) {
  const created = new Date(account.created_at);
  const createdStr = created.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
  return (
    <Flex
      align="center"
      gap={4}
      bg="white"
      borderRadius="card"
      border="1px solid"
      borderColor={isActive ? 'brand.500' : 'brand.100'}
      boxShadow={isActive ? 'glow' : 'card'}
      px={4}
      py={3.5}
      transition="all 0.15s"
    >
      <Avatar size="sm" name={account.username} bg="brand.100" color="brand.600" fontWeight="800" />
      <Box flex={1} minW={0}>
        <HStack spacing={2}>
          <Text fontWeight="800" fontSize="md" color="gray.800" noOfLines={1}>
            {account.username}
          </Text>
          {isActive ? (
            <Badge colorScheme="grass" variant="solid">
              <HStack spacing={1}>
                <CheckCircleIcon w="10px" h="10px" />
                <Text>当前</Text>
              </HStack>
            </Badge>
          ) : (
            <Badge colorScheme="gray" variant="subtle">
              离线
            </Badge>
          )}
        </HStack>
        <Text fontSize="xs" color="gray.400" mt={0.5} fontFamily="mono" noOfLines={1}>
          UUID {account.id}
        </Text>
        <Text fontSize="xs" color="gray.500" mt={0.5}>
          创建于 {createdStr}
        </Text>
      </Box>
      {!isActive && (
        <Button
          size="sm"
          variant="outline"
          colorScheme="brand"
          onClick={onActivate}
          isLoading={activating}
        >
          设为当前
        </Button>
      )}
      <Button
        size="sm"
        colorScheme="red"
        variant="ghost"
        leftIcon={<DeleteIcon />}
        onClick={onRemove}
        isLoading={removing}
      >
        删除
      </Button>
    </Flex>
  );
}
