import { Link, Icon, Text, HStack, Tooltip, Box } from '@chakra-ui/react';
import NextLink from 'next/link';
import React from 'react';
import styles from '../styles/header.module.css'

interface Props {
    url: string;
    text: string;
  }
  
export const HeaderLink = ({ text, url }: Props) => {
    return (
      <Box as="li" listStyleType="none">
          <Link href={url} className={styles.link}>
            <a>{text}</a>
          </Link>
      </Box>
    )
}