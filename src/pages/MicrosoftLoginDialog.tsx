/** M3 / L2:微软账号登录弹窗
 *
 *  - Chakra v2 Modal,中央显示
 *  - `polling` 状态显示大字号 user_code + 复制按钮 + 「打开 microsoft.com/devicelogin」按钮
 *  - 倒计时显示 `expiresIn`(后端 900s 默认,每 5s 减 5)
 *  - 终态(success / declined / expired / failed)显示对应 Alert
 *  - `useEffect(isOpen)` 触发 `start()`,关闭时 `cancel()`
 */

import {
  Alert,
  AlertIcon,
  Button,
  Code,
  HStack,
  IconButton,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Spinner,
  Text,
  VStack,
  useToast,
} from '@chakra-ui/react';
import { CopyIcon, ExternalLinkIcon } from '@chakra-ui/icons';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useEffect, useRef } from 'react';
import { useMicrosoftLogin } from '../hooks/useMicrosoftLogin';

export default function MicrosoftLoginDialog({
  isOpen,
  onClose,
  onSuccess,
}: {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (accountId: string) => void;
}) {
  const { state, start, cancel } = useMicrosoftLogin();
  const toast = useToast();
  const handledRef = useRef(false);

  // 弹窗打开触发 start;关闭 cancel
  useEffect(() => {
    if (isOpen) {
      handledRef.current = false;
      void start();
    } else {
      cancel();
    }
  }, [isOpen, start, cancel]);

  // success 一次触发 toast + onSuccess(用 ref 防止 effect 重复)
  useEffect(() => {
    if (state.status === 'success' && !handledRef.current) {
      handledRef.current = true;
      toast({
        status: 'success',
        description: `已登录 ${state.account.username}`,
        duration: 3000,
        isClosable: true,
      });
      onSuccess(state.account.id);
    }
  }, [state, toast, onSuccess]);

  const copyUserCode = async () => {
    if (state.status !== 'polling') return;
    try {
      await navigator.clipboard.writeText(state.userCode);
      toast({ status: 'info', description: '设备码已复制', duration: 1500 });
    } catch (e) {
      toast({ status: 'error', description: `复制失败: ${String(e)}` });
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      isCentered
      size="lg"
      closeOnOverlayClick={false}
    >
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>微软账号登录</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          {state.status === 'idle' && (
            <VStack py={6}>
              <Text color="gray.500">准备中...</Text>
            </VStack>
          )}

          {state.status === 'requesting' && (
            <VStack py={6}>
              <Spinner />
              <Text>正在请求设备码...</Text>
            </VStack>
          )}

          {state.status === 'polling' && (
            <VStack align="stretch" spacing={4} py={2}>
              <Text fontSize="sm">在浏览器打开下面的链接,输入设备码完成授权:</Text>
              <HStack justify="center">
                <Code
                  fontSize="2xl"
                  px={6}
                  py={3}
                  letterSpacing="widest"
                  fontFamily="mono"
                >
                  {state.userCode}
                </Code>
                <IconButton
                  aria-label="复制设备码"
                  icon={<CopyIcon />}
                  onClick={() => void copyUserCode()}
                  variant="ghost"
                />
              </HStack>
              <Button
                leftIcon={<ExternalLinkIcon />}
                onClick={() => void openUrl(state.verificationUri)}
                colorScheme="brand"
              >
                打开 microsoft.com/devicelogin
              </Button>
              <Text fontSize="xs" color="gray.500" textAlign="center">
                设备码将在 {state.expiresIn} 秒后过期
              </Text>
            </VStack>
          )}

          {state.status === 'success' && (
            <Alert status="success" borderRadius="md">
              <AlertIcon />
              登录成功:{state.account.username}
            </Alert>
          )}

          {state.status === 'declined' && (
            <Alert status="warning" borderRadius="md">
              <AlertIcon />
              用户已拒绝授权
            </Alert>
          )}

          {state.status === 'expired' && (
            <Alert status="warning" borderRadius="md">
              <AlertIcon />
              设备码已过期,请重新发起登录
            </Alert>
          )}

          {state.status === 'failed' && (
            <Alert status="error" borderRadius="md">
              <AlertIcon />
              {state.message}
            </Alert>
          )}
        </ModalBody>
        <ModalFooter>
          {state.status === 'polling' && (
            <Button variant="ghost" mr={3} onClick={cancel}>
              停止轮询
            </Button>
          )}
          {(state.status === 'declined' ||
            state.status === 'expired' ||
            state.status === 'failed') && (
            <Button colorScheme="brand" mr={3} onClick={() => void start()}>
              重新登录
            </Button>
          )}
          <Button onClick={onClose}>关闭</Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
